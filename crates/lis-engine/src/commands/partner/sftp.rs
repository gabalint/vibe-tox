//! The LIS's public SSH key for partners' SFTP servers, and testing a server as entered in
//! the connection dialog. Network work never runs in a transaction.

use std::sync::Arc;

use lis_auth::DataKey;
use lis_core::{CommandError, ErrorCode};
use lis_interfaces::partner::{
    Destination, IntakeSettings, MovedTo, PartnerConnectionId, SftpAuth, SftpSettings,
};
use rand::TryRng;
use rand::rngs::SysRng;
use russh::keys::PrivateKey;
use russh::keys::ssh_key::LineEnding;
use russh::keys::ssh_key::private::Ed25519Keypair;
use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;
use ts_rs::TS;
use zeroize::Zeroizing;

use super::folders::{FolderTestResult, blocking};
use super::{GET_LIS_SFTP_KEY, TEST_PARTNER_SFTP};
use crate::engine::Engine;
use crate::partner_delivery::ProbeProblem;
use crate::partner_delivery::sftp::{self, Credential, KEY_CONTEXT};
use crate::pipeline::Failure;

/// The comment of the LIS's public key, as the partner's IT sees it.
const KEY_COMMENT: &str = "openbga-tox";

/// Input of `test_partner_sftp`: the server as entered in the connection dialog.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct TestPartnerSftpInput {
    pub server: SftpSettings,
    /// The password as typed, for password sign-in.
    #[serde(default)]
    #[ts(optional, type = "string")]
    pub password: Option<SecretString>,
    /// The connection being changed: without a typed password, its kept password signs in
    /// to the same server, port and user.
    #[serde(default)]
    #[ts(optional)]
    pub connection_id: Option<PartnerConnectionId>,
    /// For incoming orders: the server's Processed and Failed folders, where a probe file
    /// is written; the input folder is only listed.
    #[serde(default)]
    #[ts(optional)]
    pub intake: Option<IntakeSettings>,
}

impl Engine {
    /// `get_lis_sftp_key`: the LIS's public SSH key, in OpenSSH form, for the partner's IT
    /// to allow. The first call makes the installation's key; its private key is kept
    /// sealed with the installation key and never leaves the engine.
    pub async fn get_lis_sftp_key(&self) -> Result<String, CommandError> {
        self.run(&GET_LIS_SFTP_KEY, |call| async move {
            self.authorize(&call).await?;
            let current = self
                .read(&call, lis_interfaces::partner::sftp_public_key)
                .await?;
            if let Some(public_key) = current {
                return Ok(public_key);
            }
            let (public_key, sealed) = new_identity(&self.data_key().await?)?;
            let kept = public_key.clone();
            self.write(&call, move |ctx| {
                lis_interfaces::partner::create_sftp_identity(ctx, &kept, &sealed)
            })
            .await?;
            Ok(public_key)
        })
        .await
    }

    /// `test_partner_sftp`: connects to the server as entered, checks its fingerprint and
    /// the sign-in, and writes and removes a probe file in its folder. Saves nothing.
    pub async fn test_partner_sftp(
        &self,
        input: TestPartnerSftpInput,
    ) -> Result<FolderTestResult, CommandError> {
        self.run(&TEST_PARTNER_SFTP, |call| async move {
            self.authorize(&call).await?;
            let server = lis_interfaces::partner::valid_sftp_server(input.server)?;
            let typed = input
                .password
                .filter(|password| !password.expose_secret().is_empty());
            let kept = match (typed.is_none(), input.connection_id) {
                (true, Some(id)) => kept_password(
                    &server,
                    self.connection_folder(&call, id).await?.destination,
                ),
                _ => None,
            };
            let moved_to = input.intake.map(server_folders).transpose()?;
            let key = self.data_key().await.ok();
            let db = Arc::clone(&self.db);
            let tested = blocking(move || {
                let credential = match (server.auth, typed) {
                    (SftpAuth::Password, Some(password)) => Ok(Credential::Password(
                        Zeroizing::new(password.expose_secret().to_owned()),
                    )),
                    (auth, _) => sftp::credential(&db, key.as_ref(), auth, kept.as_deref()),
                }?;
                sftp::probe_blocking(&server, &credential, moved_to.as_ref())
            })
            .await?;
            Ok(FolderTestResult::probed(tested.map_err(ProbeProblem::Sftp)))
        })
        .await
    }
}

/// The server's Processed and Failed folders as entered, each from the server's root.
fn server_folders(intake: IntakeSettings) -> Result<MovedTo, Failure> {
    let from_root = |folder: Option<String>| {
        folder
            .map(|folder| folder.trim().to_owned())
            .filter(|folder| folder.starts_with('/') && !folder.split('/').any(|part| part == ".."))
            .ok_or_else(|| {
                Failure::refused(
                    ErrorCode::Validation,
                    "Enter the Processed and Failed folders on the server from its root.",
                )
            })
    };
    Ok(MovedTo {
        processed: from_root(intake.remote_processed_folder)?,
        failed: from_root(intake.remote_failed_folder)?,
    })
}

/// The connection's sealed password, when it signs in to the same server, port and user.
fn kept_password(server: &SftpSettings, current: Destination) -> Option<Vec<u8>> {
    let same = current.sftp.is_some_and(|current| {
        current.host == server.host
            && current.port == server.port
            && current.username == server.username
    });
    current.sealed_password.filter(|_| same)
}

/// A new Ed25519 key: its public key in OpenSSH form and its private key sealed with the
/// installation key.
fn new_identity(key: &DataKey) -> Result<(String, Vec<u8>), Failure> {
    let internal = || Failure::Internal("the SSH key could not be made".to_owned());
    let mut seed = Zeroizing::new([0_u8; 32]);
    SysRng
        .try_fill_bytes(seed.as_mut())
        .map_err(|_| internal())?;
    let mut private = PrivateKey::from(Ed25519Keypair::from_seed(&seed));
    private.set_comment(KEY_COMMENT);
    let public_key = private.public_key().to_openssh().map_err(|_| internal())?;
    let pem = private.to_openssh(LineEnding::LF).map_err(|_| internal())?;
    let sealed = lis_auth::seal_secret(key, KEY_CONTEXT, pem.as_bytes()).map_err(Failure::from)?;
    Ok((public_key, sealed))
}
