//! The file template page (result files brief §7, §9, §13; REQ-ANA-003,
//! REQ-INT-005): open a sample export, show its first lines as cells, check a
//! layout against it, and register the file source.
//!
//! The sample's bytes stay in memory only, for the session that opened
//! them: at most [`MAX_SAMPLES`] per session, dropped when the session locks
//! or ends (brief §7). Opening, previewing and checking read no LIS record,
//! so they append no access event; they need `manage_configuration` or
//! `approve_configuration` (the approver runs the same check before
//! approving). The registry has no either-of access, so they are registered
//! as `Session` and checked by [`Engine::require_template_access`], which
//! audits a denial like any other. A sample may hold patient labels and
//! values: nothing here logs its content, its name or its path.

use std::fs::File;
use std::io::Read;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError};

use lis_analytical::{
    FileDelimiter, FileEncoding, FileLayout, RegisterFileSourceInput, SourceRegistrationView,
    SourceTestCodeInput, actions,
};
use lis_core::{Capability, CommandError, ErrorCode, FieldError, Id, SessionId};
use lis_interfaces::MAX_RESULT_FILE_BYTES;
use lis_results::delimited::{self, FileExtractFailure, LayoutCheck, PreviewLine};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::sign_in::session_of;
use crate::engine::Engine;
use crate::pipeline::{Call, Failure, domain_failure};
use crate::registry::{Access, CommandSpec, spec};
use crate::session::SessionState;

/// Either-of access, checked by [`Engine::require_template_access`].
pub(crate) const OPEN_LAYOUT_SAMPLE: CommandSpec = spec(
    "open_layout_sample",
    "analytical.open_layout_sample",
    Access::Session,
);
/// Either-of access, checked by [`Engine::require_template_access`].
pub(crate) const PREVIEW_LAYOUT_SAMPLE: CommandSpec = spec(
    "preview_layout_sample",
    "analytical.preview_layout_sample",
    Access::Session,
);
/// Either-of access, checked by [`Engine::require_template_access`].
pub(crate) const CHECK_FILE_LAYOUT: CommandSpec = spec(
    "check_file_layout",
    "analytical.check_file_layout",
    Access::Session,
);
pub(crate) const REGISTER_FILE_SOURCE: CommandSpec = spec(
    "register_file_source",
    actions::REGISTER_SOURCE,
    Access::Capability(Capability::ManageConfiguration),
);

/// Sample files one session may hold open at a time.
const MAX_SAMPLES: usize = 2;

/// Marker for [`LayoutSampleHandle`].
pub enum LayoutSampleMarker {}
/// A sample file held open for the template page; meaningful only to the
/// session that opened it.
pub type LayoutSampleHandle = Id<LayoutSampleMarker>;

/// An opened sample file (brief §7, step 1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LayoutSampleOpened {
    /// The handle for preview and check.
    pub sample_handle: LayoutSampleHandle,
    /// The encoding a byte-order mark decides; the person cannot change it.
    pub bom_encoding: Option<FileEncoding>,
    /// Whether the file is valid UTF-8; when not, the lines show replacement
    /// characters and Windows-1252 may read it.
    pub valid_utf8: bool,
    /// The first 50 lines, without line ends; line numbers start at 1.
    pub lines: Vec<String>,
}

/// Input of `preview_layout_sample`.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[ts(export)]
pub struct PreviewLayoutSampleInput {
    /// The open sample.
    pub sample_handle: LayoutSampleHandle,
    /// The encoding to read it with, when it has no byte-order mark.
    pub encoding: FileEncoding,
    /// The delimiter to split it with.
    pub delimiter: FileDelimiter,
}

/// Input of `check_file_layout`.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[ts(export)]
pub struct CheckFileLayoutInput {
    /// The open sample.
    pub sample_handle: LayoutSampleHandle,
    /// The layout to read it with.
    pub layout: FileLayout,
    /// The analyte names mapped so far.
    pub test_codes: Vec<SourceTestCodeInput>,
}

struct Sample {
    handle: LayoutSampleHandle,
    session: SessionId,
    bytes: Arc<Vec<u8>>,
}

/// The sample files open on template pages, in memory only.
#[derive(Default)]
pub(crate) struct LayoutSamples(Mutex<Vec<Sample>>);

impl LayoutSamples {
    fn samples(&self) -> std::sync::MutexGuard<'_, Vec<Sample>> {
        // A poisoned lock still holds a coherent list.
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Keeps `bytes` for `session`, dropping other sessions' samples and the
    /// session's oldest beyond [`MAX_SAMPLES`].
    fn insert(&self, session: SessionId, bytes: Vec<u8>) -> LayoutSampleHandle {
        let mut samples = self.samples();
        samples.retain(|sample| sample.session == session);
        while samples.len() >= MAX_SAMPLES {
            samples.remove(0);
        }
        let handle = LayoutSampleHandle::generate();
        samples.push(Sample {
            handle,
            session,
            bytes: Arc::new(bytes),
        });
        handle
    }

    fn get(&self, session: SessionId, handle: LayoutSampleHandle) -> Option<Arc<Vec<u8>>> {
        self.samples()
            .iter()
            .find(|sample| sample.handle == handle && sample.session == session)
            .map(|sample| Arc::clone(&sample.bytes))
    }

    /// Drops every sample: the session locked or ended.
    pub(crate) fn clear(&self) {
        self.samples().clear();
    }
}

impl Engine {
    /// `open_layout_sample` (dialog): reads the chosen file, at most
    /// `MAX_RESULT_FILE_BYTES`, keeps its bytes for this session and returns
    /// its first lines. `None` when the dialog was cancelled.
    pub async fn open_layout_sample(
        &self,
        file: Option<PathBuf>,
    ) -> Result<Option<LayoutSampleOpened>, CommandError> {
        self.run(&OPEN_LAYOUT_SAMPLE, |call| async move {
            self.require_template_access(&call).await?;
            let Some(path) = file else {
                return Ok(None);
            };
            let (_, session) = session_of(&call.principal)?;
            let (bytes, text) = blocking(move || {
                let bytes = read_sample(path)?;
                let text = delimited::sample_text(&bytes).map_err(unreadable)?;
                Ok((bytes, text))
            })
            .await?;
            Ok(Some(LayoutSampleOpened {
                sample_handle: self.keep_sample(session, bytes)?,
                bom_encoding: text.bom_encoding,
                valid_utf8: text.valid_utf8,
                lines: text.lines,
            }))
        })
        .await
    }

    /// `preview_layout_sample`: the sample's first lines as cells, read with
    /// the chosen encoding and delimiter.
    pub async fn preview_layout_sample(
        &self,
        input: PreviewLayoutSampleInput,
    ) -> Result<Vec<PreviewLine>, CommandError> {
        self.run(&PREVIEW_LAYOUT_SAMPLE, |call| async move {
            let bytes = self.sample(&call, input.sample_handle).await?;
            blocking(move || {
                delimited::preview_lines(&bytes, input.encoding, input.delimiter)
                    .map_err(unreadable)
            })
            .await
        })
        .await
    }

    /// `check_file_layout`: reads the sample under `layout` as an import
    /// would, row by row. Writes nothing.
    pub async fn check_file_layout(
        &self,
        input: CheckFileLayoutInput,
    ) -> Result<LayoutCheck, CommandError> {
        self.run(&CHECK_FILE_LAYOUT, |call| async move {
            let bytes = self.sample(&call, input.sample_handle).await?;
            input.layout.validate().map_err(invalid)?;
            blocking(move || {
                Ok(delimited::check_layout(
                    &bytes,
                    &input.layout,
                    &input.test_codes,
                ))
            })
            .await
        })
        .await
    }

    /// `register_file_source` (`manage_configuration`): a draft file source
    /// that another user approves with `approve_source`.
    pub async fn register_file_source(
        &self,
        input: RegisterFileSourceInput,
    ) -> Result<SourceRegistrationView, CommandError> {
        self.run(&REGISTER_FILE_SOURCE, |call| async move {
            self.write(&call, move |ctx| {
                lis_analytical::register_file_source(ctx, input)
            })
            .await
        })
        .await
    }

    /// Keeps a sample for `session` only while it is still the window's unlocked
    /// session. The session lock is held across the check and the insert, and every
    /// lock or end clears the samples under that same lock, so a sample read while the
    /// session locked or ended never lands in the cache afterwards.
    fn keep_sample(
        &self,
        session: SessionId,
        bytes: Vec<u8>,
    ) -> Result<LayoutSampleHandle, Failure> {
        let state = self.session_state();
        match *state {
            SessionState::Active(active) if active.id == session => {
                Ok(self.layout_samples.insert(session, bytes))
            }
            SessionState::Active(_) | SessionState::Locked(_) | SessionState::None => {
                Err(Failure::refused(
                    ErrorCode::SessionLocked,
                    "Your session was locked while the file was read. Unlock it and open the file again.",
                ))
            }
        }
    }

    /// `manage_configuration` or `approve_configuration`; a caller with
    /// neither is refused and the denial audited (of `manage_configuration`).
    pub(crate) async fn require_template_access(&self, call: &Call) -> Result<(), Failure> {
        let held = [
            Capability::ManageConfiguration,
            Capability::ApproveConfiguration,
        ]
        .into_iter()
        .any(|capability| call.principal.require(capability).is_ok());
        if held {
            return Ok(());
        }
        self.write(call, |ctx| ctx.require(Capability::ManageConfiguration))
            .await
    }

    /// The caller's open sample, after the access check.
    async fn sample(
        &self,
        call: &Call,
        handle: LayoutSampleHandle,
    ) -> Result<Arc<Vec<u8>>, Failure> {
        self.require_template_access(call).await?;
        let (_, session) = session_of(&call.principal)?;
        self.layout_samples.get(session, handle).ok_or_else(|| {
            Failure::refused(
                ErrorCode::NotFound,
                "The sample file is no longer open. Open it again.",
            )
        })
    }
}

/// Runs file reading or parsing off the async runtime.
async fn blocking<T, F>(work: F) -> Result<T, Failure>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, Failure> + Send + 'static,
{
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|_| Failure::Internal("layout sample task stopped".to_owned()))?
}

/// The file's bytes, refused over the size limit (F14).
fn read_sample(path: PathBuf) -> Result<Vec<u8>, Failure> {
    let mut bytes = Vec::new();
    File::open(path)
        .and_then(|file| {
            file.take(MAX_RESULT_FILE_BYTES.saturating_add(1))
                .read_to_end(&mut bytes)
        })
        .map_err(|_| Failure::refused(ErrorCode::Validation, "The file could not be read."))?;
    if u64::try_from(bytes.len()).map_or(true, |len| len > MAX_RESULT_FILE_BYTES) {
        return Err(Failure::refused(
            ErrorCode::Validation,
            "The file is larger than the 20 MiB the LIS reads.",
        ));
    }
    Ok(bytes)
}

/// A whole-file reading failure, with its stable code for the page.
fn unreadable(failure: FileExtractFailure) -> Failure {
    Failure::Refused {
        code: ErrorCode::Validation,
        message: failure.to_string(),
        field_errors: vec![FieldError {
            field: "sampleHandle".to_owned(),
            code: failure.code().to_owned(),
            message: failure.to_string(),
        }],
        gate_reasons: Vec::new(),
    }
}

fn invalid(error: FieldError) -> Failure {
    domain_failure(
        ErrorCode::Validation,
        error.message.clone(),
        vec![error],
        Vec::new(),
    )
}
