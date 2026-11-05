//! An HL7 Orders connection's input folder on the partner's SFTP server, for the length of
//! one pass: one signed-in session lists, reads and moves the files. Blocking: each step
//! runs on the engine's runtime from the intake worker's thread.

use lis_interfaces::partner::{MovedTo, SftpSettings};
use russh_sftp::client::SftpSession;
use tokio::runtime::Handle;

use super::{Credential, Open, SftpProblem, folder_at, missing, open, problem};
use crate::partner_delivery::WriteProblem;
use crate::partner_intake::candidate;

/// Names tried before giving up on a free one, as in a folder.
const MAX_SAME_NAME: u32 = 100;

/// A signed-in session on the server's input folder.
pub(crate) struct RemoteInbox {
    open: Open,
    input: String,
    moved_to: MovedTo,
    runtime: Handle,
}

impl RemoteInbox {
    /// Connects, checks the fingerprint, signs in and finds the input folder.
    pub(crate) fn open(
        server: &SftpSettings,
        credential: Credential,
        moved_to: MovedTo,
    ) -> Result<Self, SftpProblem> {
        let runtime = Handle::current();
        let open = runtime.block_on(open(server, &credential))?;
        let input = runtime.block_on(folder_at(&open.sftp, &server.remote_folder))?;
        Ok(Self {
            open,
            input,
            moved_to,
            runtime,
        })
    }

    /// The regular files in the input folder.
    pub(crate) fn names(&mut self) -> Result<Vec<String>, SftpProblem> {
        let entries = self
            .runtime
            .block_on(self.open.sftp.read_dir(self.input.clone()))
            .map_err(|error| problem(&error))?;
        Ok(entries
            .filter(|entry| entry.file_type().is_file())
            .map(|entry| entry.file_name())
            .collect())
    }

    pub(crate) fn read(&mut self, name: &str) -> Result<Vec<u8>, SftpProblem> {
        let path = format!("{}/{name}", self.input);
        self.runtime
            .block_on(self.open.sftp.read(path))
            .map_err(|error| problem(&error))
    }

    /// Moves the file to Processed (or Failed) › `year` › `month` under a free name; an SFTP
    /// rename never replaces a file.
    pub(crate) fn move_out(
        &mut self,
        name: &str,
        failed: bool,
        year: &str,
        month: &str,
    ) -> Result<String, SftpProblem> {
        let root = if failed {
            &self.moved_to.failed
        } else {
            &self.moved_to.processed
        };
        let year_folder = format!("{}/{year}", root.trim_end_matches('/'));
        let folder = format!("{year_folder}/{month}");
        let source = format!("{}/{name}", self.input);
        let sftp = &self.open.sftp;
        self.runtime.block_on(async {
            ensure_folder(sftp, &year_folder).await?;
            ensure_folder(sftp, &folder).await?;
            place(sftp, &source, &folder, name).await
        })
    }
}

/// Renames `source` into `folder` under the first free name.
async fn place(
    sftp: &SftpSession,
    source: &str,
    folder: &str,
    name: &str,
) -> Result<String, SftpProblem> {
    for n in 0..MAX_SAME_NAME {
        let target = format!("{folder}/{}", candidate(name, n));
        if sftp.try_exists(target.as_str()).await.unwrap_or(false) {
            continue;
        }
        match sftp.rename(source, target.as_str()).await {
            Ok(()) => return Ok(target),
            Err(_) if sftp.try_exists(target.as_str()).await.unwrap_or(false) => {}
            Err(error) => return Err(problem(&error)),
        }
    }
    Err(SftpProblem::Write(WriteProblem::NameTaken))
}

/// Makes `path` if it does not exist yet.
async fn ensure_folder(sftp: &SftpSession, path: &str) -> Result<(), SftpProblem> {
    match sftp.metadata(path).await {
        Ok(metadata) if metadata.is_dir() => Ok(()),
        Ok(_) => Err(SftpProblem::Write(WriteProblem::FolderMissing)),
        Err(error) if missing(&error) => {
            sftp.create_dir(path).await.map_err(|error| problem(&error))
        }
        Err(error) => Err(problem(&error)),
    }
}
