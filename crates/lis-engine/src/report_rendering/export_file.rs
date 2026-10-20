//! The one path that publishes an export file, for report PDFs and billing
//! handoffs (review findings 10 and B2; NFR-07). A crash at any step leaves
//! no file, or a file named by an audited intent:
//!
//! 1. reserve an empty `<name>.partial` beside the first free `<name>`;
//! 2. commit an export-intent event in its own transaction (security stream:
//!    action, exported record, intended file name and digest; no PHI);
//! 3. write and sync the bytes into the reserved file;
//! 4. publish it as `<name>` without replacing anything: a hard link fails
//!    when the name exists; then the `.partial` name is removed and the
//!    folder synced;
//! 5. record the export.
//!
//! A file left by a failed step is removed; one that cannot be removed is
//! named in a residual-file security event.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use lis_audit::{AuditDetail, AuditEvent, Outcome, Stream};
use lis_store::{CommandContext, StoreError, sync_dir};

use crate::engine::Engine;
use crate::pipeline::{Call, Failure, ModuleError};

/// Files of one name stem in one folder, at most.
const MAX_SAME_NAME: u32 = 100;

/// What is exported and where: the Downloads folder the desktop shell
/// resolves, a PHI-free name stem, and the digest of the verified bytes.
pub(crate) struct ExportFile {
    pub(crate) folder: PathBuf,
    pub(crate) stem: String,
    pub(crate) extension: &'static str,
    pub(crate) sha256: String,
    /// The exported record, named in the intent event, such as `("handoff_id", id)`.
    pub(crate) source: (&'static str, AuditDetail),
}

/// An empty `.partial` file reserved for one export.
struct Reserved {
    file: File,
    partial: PathBuf,
    target: PathBuf,
    name: String,
}

impl Engine {
    /// Publishes `bytes` as a new file and records it with `record`, which is
    /// given the published file name. Returns that name.
    pub(crate) async fn export_file<T, E, R>(
        &self,
        call: &Call,
        file: ExportFile,
        bytes: Vec<u8>,
        record: R,
    ) -> Result<String, Failure>
    where
        T: Send + 'static,
        E: ModuleError,
        R: FnOnce(&mut CommandContext<'_>, &str) -> Result<T, E> + Send + 'static,
    {
        let fill = move |file: &mut File| {
            file.write_all(&bytes)?;
            file.sync_all()
        };
        self.export_with(call, file, fill, record).await
    }

    /// [`Engine::export_file`] with the write-and-sync step `fill`, which
    /// tests replace to inject failures.
    async fn export_with<T, E, R>(
        &self,
        call: &Call,
        file: ExportFile,
        fill: impl FnOnce(&mut File) -> io::Result<()> + Send + 'static,
        record: R,
    ) -> Result<String, Failure>
    where
        T: Send + 'static,
        E: ModuleError,
        R: FnOnce(&mut CommandContext<'_>, &str) -> Result<T, E> + Send + 'static,
    {
        let ExportFile {
            folder,
            stem,
            extension,
            sha256,
            source,
        } = file;
        let reserve_in = folder.clone();
        let reserved = blocking(move || reserve(&reserve_in, &stem, extension)).await?;
        let (partial, target, name) = (
            reserved.partial.clone(),
            reserved.target.clone(),
            reserved.name.clone(),
        );
        let intent = intent_event(call.spec.action, &name, &sha256, source);
        let intended = self
            .write(call, move |ctx| ctx.audit(intent?).map(drop))
            .await;
        if let Err(failure) = intended {
            self.discard_unrecorded(call, &partial, &sha256).await;
            return Err(failure);
        }
        let published = blocking(move || publish(reserved, fill)).await;
        // The `.partial` name goes in every case: after publication it is a
        // second name of the file, otherwise the unpublished bytes.
        self.discard_unrecorded(call, &partial, &sha256).await;
        published?;
        // From here the published file is ours: any failure removes it.
        let recorded = match blocking(move || sync_dir(&folder)).await {
            Ok(()) => {
                let recorded_name = name.clone();
                self.write(call, move |ctx| record(ctx, &recorded_name).map(drop))
                    .await
            }
            Err(failure) => Err(failure),
        };
        if recorded.is_err() {
            self.discard_unrecorded(call, &target, &sha256).await;
        }
        recorded.map(|()| name)
    }

    /// Removes an export file that must not stay: unpublished bytes, or a
    /// file whose recording failed. If it cannot be removed, a security
    /// event names the residual file and its digest (NFR-07); the export's
    /// own error is returned.
    async fn discard_unrecorded(&self, call: &Call, path: &Path, sha256: &str) {
        let error = match fs::remove_file(path) {
            Ok(()) => return,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return,
            Err(error) => error,
        };
        tracing::error!(%error, "unrecorded export file not removed");
        let event = file_event(call.spec.action, Outcome::Failed, sha256)
            .map(|event| event.record("residual_export_file", file_name(path)));
        let recorded: Result<(), StoreError> = self
            .db
            .write(call.principal.clone(), call.correlation_id, move |ctx| {
                ctx.audit(event?).map(drop)
            })
            .await;
        if let Err(error) = recorded {
            tracing::error!(%error, "residual export file not audited");
        }
    }
}

/// The intent to publish `name`, committed before any byte is written.
fn intent_event(
    action: &'static str,
    name: &str,
    sha256: &str,
    (key, record): (&'static str, AuditDetail),
) -> Result<AuditEvent, StoreError> {
    Ok(file_event(action, Outcome::Succeeded, sha256)?
        .record("export_intent", name)
        .detail(key, record))
}

/// A security event about an export file, with the bytes' digest and the
/// folder. Its record ID is the file name, built from a report number or a
/// handoff ID (PHI-free).
fn file_event(
    action: &'static str,
    outcome: Outcome,
    sha256: &str,
) -> Result<AuditEvent, StoreError> {
    Ok(AuditEvent::new(Stream::Security, action, outcome)
        .detail("artifact_sha256", AuditDetail::sha256(sha256)?)
        .detail("folder", AuditDetail::code("downloads")))
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

async fn blocking<T: Send + 'static>(
    step: impl FnOnce() -> io::Result<T> + Send + 'static,
) -> Result<T, Failure> {
    tokio::task::spawn_blocking(step)
        .await
        .map_err(|_| Failure::Internal("export task stopped".to_owned()))?
        .map_err(|error| Failure::Internal(format!("export not written: {error}")))
}

/// Reserves `<stem>.<extension>`, or `<stem>-<n>.<extension>` when that or
/// its `.partial` exists, by creating its empty `.partial` file.
fn reserve(folder: &Path, stem: &str, extension: &str) -> io::Result<Reserved> {
    for n in 0..MAX_SAME_NAME {
        let name = if n == 0 {
            format!("{stem}.{extension}")
        } else {
            format!("{stem}-{n}.{extension}")
        };
        let target = folder.join(&name);
        if target.try_exists()? {
            continue;
        }
        let partial = folder.join(format!("{name}.partial"));
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&partial)
        {
            Ok(file) => {
                return Ok(Reserved {
                    file,
                    partial,
                    target,
                    name,
                });
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "too many exports with the same name",
    ))
}

/// Writes the reserved file with `fill`, then gives it its final name
/// without replacing any file there.
fn publish(
    mut reserved: Reserved,
    fill: impl FnOnce(&mut File) -> io::Result<()>,
) -> io::Result<()> {
    fill(&mut reserved.file)?;
    drop(reserved.file);
    fs::hard_link(&reserved.partial, &reserved.target)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use lis_auth::DataKey;
    use lis_core::{CorrelationId, FixedClock, Principal, UtcTimestamp};
    use lis_reporting::ReportRevisionId;

    use super::*;
    use crate::commands::reporting::EXPORT_REPORT_PDF;
    use crate::{EngineConfig, KeyProvider};

    const BYTES: &[u8] = b"%PDF synthetic";
    const STEM: &str = "A26000001-SYN-UR-r1";

    async fn engine(dir: &Path) -> Engine {
        let start = UtcTimestamp::parse("2026-10-13T10:00:00.000000000Z").unwrap();
        Engine::start(EngineConfig {
            data_dir: dir.to_owned(),
            clock: Arc::new(FixedClock::new(start)),
            keys: KeyProvider::Fixed(DataKey::from_bytes([7; 32])),
            printer_destinations: crate::PrinterDestinations::LabNetwork,
            webview: None,
            outbox_handlers: Vec::new(),
            installation_kind: crate::InstallationKind::Laboratory,
        })
        .await
        .unwrap()
    }

    fn call() -> Call {
        Call {
            spec: &EXPORT_REPORT_PDF,
            correlation_id: CorrelationId::generate(),
            principal: Principal::system("export-test", []),
        }
    }

    fn file(folder: &Path, sha256: &str) -> ExportFile {
        ExportFile {
            folder: folder.to_owned(),
            stem: STEM.to_owned(),
            extension: "pdf",
            sha256: sha256.to_owned(),
            source: ("revision_id", AuditDetail::id(ReportRevisionId::generate())),
        }
    }

    fn digest() -> String {
        "ab".repeat(32)
    }

    fn recorded(_: &mut CommandContext<'_>, _: &str) -> Result<(), StoreError> {
        Ok(())
    }

    /// `(outcome, record_type, record_id)` of the security events, in order.
    async fn file_events(engine: &Engine) -> Vec<(String, String, String)> {
        engine
            .db
            .read(|conn| {
                let mut statement = conn.prepare(
                    "SELECT outcome, record_type, record_id FROM audit_events \
                     WHERE action = ?1 ORDER BY seq",
                )?;
                let rows = statement
                    .query_map([EXPORT_REPORT_PDF.action], |row| {
                        Ok((row.get(0)?, row.get(1)?, row.get(2)?))
                    })?
                    .collect::<Result<Vec<_>, _>>()?;
                Ok::<_, StoreError>(rows)
            })
            .await
            .unwrap()
    }

    fn names(folder: &Path) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(folder)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    fn intent(name: &str) -> (String, String, String) {
        (
            "succeeded".to_owned(),
            "export_intent".to_owned(),
            name.to_owned(),
        )
    }

    fn fail_write(file: &mut File) -> io::Result<()> {
        file.write_all(&BYTES[..4])?;
        Err(io::Error::other("injected write failure"))
    }

    fn fail_sync(file: &mut File) -> io::Result<()> {
        file.write_all(BYTES)?;
        Err(io::Error::other("injected sync failure"))
    }

    // Findings 10 and B2 (NFR-07): the intent is committed before the file
    // is published, and nothing but the file stays in the folder.
    #[tokio::test]
    async fn an_export_publishes_after_its_intent() {
        let (data, downloads) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        let engine = engine(data.path()).await;
        let name = engine
            .export_file(
                &call(),
                file(downloads.path(), &digest()),
                BYTES.to_vec(),
                recorded,
            )
            .await
            .unwrap();
        assert_eq!(name, format!("{STEM}.pdf"));
        assert_eq!(names(downloads.path()), std::slice::from_ref(&name));
        assert_eq!(fs::read(downloads.path().join(&name)).unwrap(), BYTES);
        assert_eq!(file_events(&engine).await, [intent(&name)]);
        let second = engine
            .export_file(
                &call(),
                file(downloads.path(), &digest()),
                BYTES.to_vec(),
                recorded,
            )
            .await
            .unwrap();
        assert_eq!(second, format!("{STEM}-1.pdf"));
    }

    #[tokio::test]
    async fn no_file_is_written_when_the_intent_is_not_committed() {
        let (data, downloads) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        let engine = engine(data.path()).await;
        let refused = engine
            .export_file(
                &call(),
                file(downloads.path(), "not a digest"),
                BYTES.to_vec(),
                recorded,
            )
            .await;
        assert!(refused.is_err());
        assert!(names(downloads.path()).is_empty());
        assert!(file_events(&engine).await.is_empty());
    }

    #[tokio::test]
    async fn a_failed_write_or_sync_leaves_no_file() {
        for fill in [fail_write, fail_sync] {
            let (data, downloads) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
            let engine = engine(data.path()).await;
            let failed = engine
                .export_with(&call(), file(downloads.path(), &digest()), fill, recorded)
                .await;
            assert!(failed.is_err());
            assert!(names(downloads.path()).is_empty());
            assert_eq!(file_events(&engine).await, [intent(&format!("{STEM}.pdf"))]);
        }
    }

    // Publication never replaces a file that appeared meanwhile.
    #[tokio::test]
    async fn publication_never_replaces_a_file() {
        let (data, downloads) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        let engine = engine(data.path()).await;
        let target = downloads.path().join(format!("{STEM}.pdf"));
        let racing = target.clone();
        let fill = move |file: &mut File| {
            fs::write(&racing, b"someone else's file")?;
            file.write_all(BYTES)
        };
        let failed = engine
            .export_with(&call(), file(downloads.path(), &digest()), fill, recorded)
            .await;
        assert!(failed.is_err());
        assert_eq!(names(downloads.path()), [format!("{STEM}.pdf")]);
        assert_eq!(fs::read(&target).unwrap(), b"someone else's file");
    }

    #[tokio::test]
    async fn an_unrecorded_export_is_removed() {
        let (data, downloads) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        let engine = engine(data.path()).await;
        let unrecorded =
            |_: &mut CommandContext<'_>, _: &str| Err::<(), _>(StoreError::TimeOutOfRange);
        let failed = engine
            .export_file(
                &call(),
                file(downloads.path(), &digest()),
                BYTES.to_vec(),
                unrecorded,
            )
            .await;
        assert!(failed.is_err());
        assert!(names(downloads.path()).is_empty());
        assert_eq!(file_events(&engine).await, [intent(&format!("{STEM}.pdf"))]);
    }

    // Review finding 6 (NFR-07): an export file that cannot be removed is
    // named, with its digest, on the security stream.
    #[tokio::test]
    async fn an_unremovable_export_file_is_audited_as_residual() {
        let (data, downloads) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        let engine = engine(data.path()).await;
        let removable = downloads.path().join(format!("{STEM}.pdf"));
        fs::write(&removable, BYTES).unwrap();
        engine
            .discard_unrecorded(&call(), &removable, &digest())
            .await;
        assert!(!removable.exists());
        assert!(file_events(&engine).await.is_empty());
        // Injected failure: a directory cannot be removed as a file.
        let stuck = downloads.path().join(format!("{STEM}-1.pdf"));
        fs::create_dir(&stuck).unwrap();
        engine.discard_unrecorded(&call(), &stuck, &digest()).await;
        assert_eq!(
            file_events(&engine).await,
            [(
                "failed".to_owned(),
                "residual_export_file".to_owned(),
                format!("{STEM}-1.pdf")
            )]
        );
    }
}
