//! The installation key (M4 design brief D2, §3.9; security-audit-logging.md, "Secrets and
//! credentials"): the one key that seals TOTP secrets and, through a derived key, the
//! backups. It is loaded at startup, never lazily, and its state decides what may happen:
//!
//! - a key is created only for a database that holds nothing bound to a key: no sealed
//!   secrets, no recovery kit (its key ID) and no backup target (whose marker and sets
//!   carry the key ID). A new key over such data could never open it: it would lock out
//!   every TOTP user and orphan the backups;
//! - a key that is absent from the key store is reported as `missing`; a key that opens
//!   none of the sealed secrets, or with none sealed does not have the newest kit's key
//!   ID, as `mismatch`. Listeners and work without sign-in continue, TOTP sign-in is
//!   refused, and the key line from the recovery kit restores it;
//! - a key store that fails is `unavailable`.
//!
//! Sources: the OS key store for the app, a fixed key, or an in-memory store that may be
//! empty and can be written, for tests and hosts without a key store.
//!
//! The key-store entry belongs to one app: its service name is the bundle identifier the
//! shell runs as, so a test copy or another bundle (for example
//! `org.vibetox.lis.smoketest`) on the same OS account never reads, overwrites or creates
//! the production installation's key. The production identifier's entry is the one every
//! earlier build used (service `org.vibetox.lis`), so an existing installation keeps its key
//! without a migration.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, PoisonError};

use lis_auth::{AuthError, DataKey};
use lis_core::{ErrorCode, GateReason};
use lis_ops::backup::BackupError;
use lis_ops::format::BackupKey;
use lis_store::{Database, StoreError};
use rusqlite::Connection;
use serde::Serialize;
use ts_rs::TS;

use crate::pipeline::Failure;

/// The production app's bundle identifier (`tauri.conf.json`; placeholder until Q-19).
const PRODUCTION_IDENTIFIER: &str = "org.vibetox.lis";
/// Key store account name of the installation key.
const ACCOUNT: &str = "totp-secret-key";

/// The key-store entry `(service, account)` of the app with bundle `identifier`. For
/// [`PRODUCTION_IDENTIFIER`] it is the entry earlier builds used for every identifier.
fn entry_name(identifier: &str) -> (&str, &'static str) {
    (identifier, ACCOUNT)
}

/// The source of the installation key.
#[derive(Debug, Clone)]
pub enum KeyProvider {
    /// The OS key store (Keychain, Credential Manager, Secret Service), in the entry of
    /// the app with this bundle identifier (the shell passes `app.config().identifier`).
    OsKeyStore {
        /// The bundle identifier.
        identifier: String,
    },
    /// A key held in memory that cannot be replaced, for tests and hosts without a key
    /// store. A restored key is then kept by the engine for the rest of the run.
    Fixed(DataKey),
    /// An in-memory key store that may be empty and can be written, for tests.
    Memory(MemoryKeyStore),
}

/// An in-memory key store shared with the test that created it: entries by name, as one
/// OS account's key store holds them, seen by the app with one bundle identifier.
#[derive(Debug, Clone)]
pub struct MemoryKeyStore {
    identifier: String,
    entries: Arc<Mutex<BTreeMap<(String, &'static str), DataKey>>>,
}

impl MemoryKeyStore {
    /// A store seen by the production app, holding `key` in its entry, or empty.
    #[must_use]
    pub fn new(key: Option<DataKey>) -> Self {
        let store = Self {
            identifier: PRODUCTION_IDENTIFIER.to_owned(),
            entries: Arc::default(),
        };
        if let Some(key) = key {
            store.set(&key);
        }
        store
    }

    /// The same store seen by the app with bundle `identifier`.
    #[must_use]
    pub fn app(&self, identifier: &str) -> Self {
        Self {
            identifier: identifier.to_owned(),
            entries: Arc::clone(&self.entries),
        }
    }

    /// The key in this app's entry now.
    #[must_use]
    pub fn key(&self) -> Option<DataKey> {
        let (service, account) = entry_name(&self.identifier);
        self.entries
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(&(service.to_owned(), account))
            .cloned()
    }

    fn set(&self, key: &DataKey) {
        let (service, account) = entry_name(&self.identifier);
        self.entries
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert((service.to_owned(), account), key.clone());
    }
}

/// Why the key store failed. Messages never contain key material.
#[derive(Debug, thiserror::Error)]
pub(crate) enum KeyError {
    #[error("OS key store: {0}")]
    Store(#[from] keyring::Error),
    #[error(transparent)]
    Key(#[from] AuthError),
    #[error(transparent)]
    Backup(#[from] BackupError),
}

impl From<StoreError> for KeyError {
    fn from(error: StoreError) -> Self {
        Self::Key(error.into())
    }
}

impl KeyProvider {
    /// The stored key, or `None` when the store has none. Blocking: the OS key store may
    /// make IPC calls.
    fn load(&self) -> Result<Option<DataKey>, KeyError> {
        match self {
            Self::Fixed(key) => Ok(Some(key.clone())),
            Self::Memory(store) => Ok(store.key()),
            Self::OsKeyStore { identifier } => match entry(identifier)?.get_secret() {
                Ok(bytes) => Ok(Some(DataKey::try_from(bytes.as_slice())?)),
                Err(keyring::Error::NoEntry) => Ok(None),
                Err(error) => Err(error.into()),
            },
        }
    }

    /// Writes `key` to the store. Blocking. A fixed key is not replaced.
    pub(crate) fn store(&self, key: &DataKey) -> Result<(), KeyError> {
        match self {
            Self::Fixed(_) => Ok(()),
            Self::Memory(store) => {
                store.set(key);
                Ok(())
            }
            Self::OsKeyStore { identifier } => Ok(entry(identifier)?.set_secret(key.expose())?),
        }
    }
}

fn entry(identifier: &str) -> Result<keyring::Entry, keyring::Error> {
    let (service, account) = entry_name(identifier);
    keyring::Entry::new(service, account)
}

lis_core::text_enum! {
    /// Whether the installation key is usable (§3.9).
    #[ts(export)]
    KeyState {
        /// Loaded, and it opens the sealed secrets (or none are sealed yet).
        Ok = "ok",
        /// The key store has no key, and the database holds data bound to one.
        Missing = "missing",
        /// The stored key opens none of the sealed secrets, or with none sealed it does
        /// not have the newest recovery kit's key ID.
        Mismatch = "mismatch",
        /// The key store failed.
        Unavailable = "unavailable",
    }
}

/// `get_installation_key_state` and `restore_installation_key` answer this.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct InstallationKeyStateView {
    /// The key's state.
    pub key_state: KeyState,
}

/// The key's state and, when it is `ok`, the key.
#[derive(Debug, Clone)]
pub(crate) struct InstallationKey {
    pub(crate) state: KeyState,
    pub(crate) key: Option<DataKey>,
}

impl InstallationKey {
    /// The key, or the typed refusal of anything that needs it (TOTP sign-in).
    pub(crate) fn usable(&self) -> Result<DataKey, Failure> {
        match (&self.key, self.state) {
            (Some(key), KeyState::Ok) => Ok(key.clone()),
            (_, state) => Err(Failure::Refused {
                code: ErrorCode::Unavailable,
                message: "The installation key is not available on this computer. Enter it \
                          from the recovery kit at the sign-in screen."
                    .to_owned(),
                field_errors: Vec::new(),
                gate_reasons: vec![GateReason {
                    code: format!("installation_key_{}", state.as_str()),
                    message: "Sign-in with an authenticator needs the installation key.".to_owned(),
                }],
            }),
        }
    }
}

/// Loads the key at startup and decides its state. Blocking. A key is created only when
/// the store has none and nothing in the database is bound to a key.
pub(crate) fn resolve(db: &Database, keys: &KeyProvider) -> Result<InstallationKey, KeyError> {
    let loaded = keys.load();
    let candidate = loaded.as_ref().ok().cloned().flatten();
    let (bound, belongs) = db.read_blocking(move |conn| -> Result<_, KeyError> {
        let bound = Bound::read(conn)?;
        let belongs = match &candidate {
            Some(key) => bound.belongs(conn, key)?,
            None => None,
        };
        Ok((bound, belongs))
    })?;
    let state = |state, key| InstallationKey { state, key };
    let resolved = match loaded {
        Err(error) => {
            tracing::error!(%error, "installation key store unavailable");
            state(KeyState::Unavailable, None)
        }
        Ok(Some(_)) if belongs == Some(false) => state(KeyState::Mismatch, None),
        Ok(Some(key)) => state(KeyState::Ok, Some(key)),
        Ok(None) if !bound.is_empty() => state(KeyState::Missing, None),
        Ok(None) => match create(keys) {
            Ok(key) => state(KeyState::Ok, Some(key)),
            Err(error) => {
                tracing::error!(%error, "installation key could not be created");
                state(KeyState::Unavailable, None)
            }
        },
    };
    tracing::info!(
        key_state = resolved.state.as_str(),
        "installation key loaded"
    );
    Ok(resolved)
}

/// Whether `key` belongs to this database (see [`Bound::belongs`]). Blocking.
pub(crate) fn belongs(db: &Database, key: &DataKey) -> Result<Option<bool>, KeyError> {
    let key = key.clone();
    db.read_blocking(move |conn| Bound::read(conn)?.belongs(conn, &key))
}

/// What in this database is bound to the installation key.
struct Bound {
    /// TOTP secrets sealed with it.
    sealed: bool,
    /// The key ID the newest recovery kit records.
    kit_key_id: Option<String>,
    /// A backup target exists; its marker and sets carry the key ID.
    targets: bool,
}

impl Bound {
    fn read(conn: &Connection) -> Result<Self, KeyError> {
        Ok(Self {
            sealed: lis_auth::has_sealed_secrets(conn)?,
            kit_key_id: lis_ops::kit::latest_key_fingerprint(conn)?,
            targets: !lis_ops::backup::list_targets(conn)?.is_empty(),
        })
    }

    /// Nothing is bound to a key yet: only then may a key be created.
    fn is_empty(&self) -> bool {
        !self.sealed && self.kit_key_id.is_none() && !self.targets
    }

    /// `Some(true)` when `key` opens a sealed secret or, with none sealed, has the newest
    /// kit's key ID; `Some(false)` when it does not; `None` when the database holds
    /// nothing to check it against.
    fn belongs(&self, conn: &Connection, key: &DataKey) -> Result<Option<bool>, KeyError> {
        if self.sealed {
            return Ok(Some(lis_auth::key_opens_sealed_secrets(conn, key)?));
        }
        Ok(self
            .kit_key_id
            .as_ref()
            .map(|kit| BackupKey::derive(key).key_id().to_string() == *kit))
    }
}

fn create(keys: &KeyProvider) -> Result<DataKey, KeyError> {
    let key = DataKey::generate()?;
    keys.store(&key)?;
    tracing::info!("installation key created in the key store");
    Ok(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SMOKE_TEST: &str = "org.vibetox.lis.smoketest";

    fn key(byte: u8) -> DataKey {
        DataKey::from_bytes([byte; 32])
    }

    // An installation from before per-identifier entries keeps its key: the production
    // identifier names exactly the entry those builds used.
    #[test]
    fn the_production_identifier_keeps_the_earlier_entry() {
        assert_eq!(
            entry_name(PRODUCTION_IDENTIFIER),
            ("org.vibetox.lis", "totp-secret-key")
        );
    }

    #[test]
    fn another_identifier_never_reads_or_writes_the_production_entry() {
        let production = MemoryKeyStore::new(Some(key(1)));
        let smoke_test = KeyProvider::Memory(production.app(SMOKE_TEST));
        assert!(smoke_test.load().unwrap().is_none());
        smoke_test.store(&key(2)).unwrap();
        assert_eq!(production.key().map(|k| *k.expose()), Some([1; 32]));
        assert_eq!(
            production.app(SMOKE_TEST).key().map(|k| *k.expose()),
            Some([2; 32])
        );
        assert_ne!(entry_name(SMOKE_TEST), entry_name(PRODUCTION_IDENTIFIER));
    }
}
