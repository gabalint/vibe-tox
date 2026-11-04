//! Listing, reading, adding, changing, starting, stopping and checking connections.

use lis_core::{CommandError, ErrorCode, FieldError};
use lis_interfaces::partner::{
    CreatePartnerConnection, PartnerConnection, PartnerConnectionDetail, PartnerConnectionId,
    PartnerConnectionSettings, PartnerError, RecordPartnerDestinationCheck,
    SetPartnerConnectionActive, UpdatePartnerConnection,
};
use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;
use ts_rs::TS;

use super::folder_choices::FolderChoiceId;
use super::{
    CREATE_PARTNER_CONNECTION, GET_PARTNER_CONNECTION, LIST_PARTNER_CONNECTIONS,
    RECORD_PARTNER_DESTINATION_CHECK, SET_PARTNER_CONNECTION_ACTIVE, UPDATE_PARTNER_CONNECTION,
};
use crate::commands::people::{display_names, name_of};
use crate::commands::sign_in::session_of;
use crate::engine::Engine;
use crate::pipeline::{Call, Failure};

/// Input of `get_partner_connection` and `send_partner_test_message`.
#[derive(Debug, Clone, Copy, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct PartnerConnectionInput {
    pub connection_id: PartnerConnectionId,
}

/// Input of `create_partner_connection`.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct CreatePartnerConnectionInput {
    pub settings: PartnerConnectionSettings,
    /// The folder chosen with `choose_partner_folder`; absent for SFTP.
    #[serde(default)]
    #[ts(optional, type = "string")]
    pub folder_choice: Option<FolderChoiceId>,
    /// The SFTP password, for password sign-in; sealed before it is kept.
    #[serde(default)]
    #[ts(optional, type = "string")]
    pub password: Option<SecretString>,
    /// False saves it stopped.
    pub active: bool,
}

/// Input of `update_partner_connection`.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct UpdatePartnerConnectionInput {
    pub connection_id: PartnerConnectionId,
    #[ts(type = "number")]
    pub expected_revision: i64,
    pub settings: PartnerConnectionSettings,
    /// A newly chosen folder; absent keeps the current one.
    #[serde(default)]
    #[ts(optional, type = "string")]
    pub folder_choice: Option<FolderChoiceId>,
    /// A new SFTP password; absent keeps the current one for the same server and user.
    #[serde(default)]
    #[ts(optional, type = "string")]
    pub password: Option<SecretString>,
    pub reason: String,
}

impl Engine {
    /// `list_partner_connections`: every connection by name, with how it is working.
    pub async fn list_partner_connections(&self) -> Result<Vec<PartnerConnection>, CommandError> {
        self.run(&LIST_PARTNER_CONNECTIONS, |call| async move {
            self.read(&call, named_connections).await
        })
        .await
    }

    /// `get_partner_connection`: one connection with its history.
    pub async fn get_partner_connection(
        &self,
        input: PartnerConnectionInput,
    ) -> Result<PartnerConnectionDetail, CommandError> {
        self.run(&GET_PARTNER_CONNECTION, |call| async move {
            self.read(&call, move |conn| detail(conn, input.connection_id))
                .await
        })
        .await
    }

    /// `create_partner_connection`: started, or stopped with `active` false. It writes no
    /// patient data until its destination check is recorded.
    pub async fn create_partner_connection(
        &self,
        input: CreatePartnerConnectionInput,
    ) -> Result<PartnerConnectionDetail, CommandError> {
        self.run(&CREATE_PARTNER_CONNECTION, |call| async move {
            self.authorize(&call).await?;
            let folder = self.chosen_folder(&call, input.folder_choice)?;
            let sealed_password = self.sealed_password(input.password.as_ref()).await?;
            self.write(&call, move |ctx| {
                let id = lis_interfaces::partner::create_partner_connection(
                    ctx,
                    CreatePartnerConnection {
                        settings: input.settings,
                        folder: folder.unwrap_or_default(),
                        sealed_password,
                        active: input.active,
                    },
                )?;
                detail(ctx.tx(), id)
            })
            .await
        })
        .await
    }

    /// `update_partner_connection`: new settings, and a new folder when one was chosen.
    pub async fn update_partner_connection(
        &self,
        input: UpdatePartnerConnectionInput,
    ) -> Result<PartnerConnectionDetail, CommandError> {
        self.run(&UPDATE_PARTNER_CONNECTION, |call| async move {
            self.authorize(&call).await?;
            let folder = self.chosen_folder(&call, input.folder_choice)?;
            let sealed_password = self.sealed_password(input.password.as_ref()).await?;
            self.write(&call, move |ctx| {
                let id = lis_interfaces::partner::update_partner_connection(
                    ctx,
                    UpdatePartnerConnection {
                        connection_id: input.connection_id,
                        expected_revision: input.expected_revision,
                        settings: input.settings,
                        folder,
                        sealed_password,
                        reason: input.reason,
                    },
                )?;
                detail(ctx.tx(), id)
            })
            .await
        })
        .await
    }

    /// `set_partner_connection_active`: start or stop, with a reason.
    pub async fn set_partner_connection_active(
        &self,
        input: SetPartnerConnectionActive,
    ) -> Result<PartnerConnectionDetail, CommandError> {
        self.run(&SET_PARTNER_CONNECTION_ACTIVE, |call| async move {
            self.write(&call, move |ctx| {
                let id = lis_interfaces::partner::set_partner_connection_active(ctx, input)?;
                detail(ctx.tx(), id)
            })
            .await
        })
        .await
    }

    /// `record_partner_destination_check` (REQ-RPT-012).
    pub async fn record_partner_destination_check(
        &self,
        input: RecordPartnerDestinationCheck,
    ) -> Result<PartnerConnectionDetail, CommandError> {
        self.run(&RECORD_PARTNER_DESTINATION_CHECK, |call| async move {
            self.write(&call, move |ctx| {
                let id = lis_interfaces::partner::record_partner_destination_check(ctx, input)?;
                detail(ctx.tx(), id)
            })
            .await
        })
        .await
    }

    /// An SFTP password sealed with the installation key; `None` without one.
    async fn sealed_password(
        &self,
        password: Option<&SecretString>,
    ) -> Result<Option<Vec<u8>>, Failure> {
        let Some(password) = password.filter(|password| !password.expose_secret().is_empty())
        else {
            return Ok(None);
        };
        let key = self.data_key().await?;
        lis_auth::seal_secret(
            &key,
            crate::partner_delivery::sftp::PASSWORD_CONTEXT,
            password.expose_secret().as_bytes(),
        )
        .map(Some)
        .map_err(Failure::from)
    }

    /// The folder behind `handle`, chosen in this session; `None` without a handle.
    pub(super) fn chosen_folder(
        &self,
        call: &Call,
        handle: Option<FolderChoiceId>,
    ) -> Result<Option<String>, Failure> {
        let Some(handle) = handle else {
            return Ok(None);
        };
        let (_, session) = session_of(&call.principal)?;
        self.folder_choices
            .get(session, handle)
            .map(Some)
            .ok_or_else(|| Failure::Refused {
                code: ErrorCode::Validation,
                message: "Choose the folder again.".to_owned(),
                field_errors: vec![FieldError {
                    field: "folderChoice".to_owned(),
                    code: "expired".to_owned(),
                    message: "Choose the folder again.".to_owned(),
                }],
                gate_reasons: Vec::new(),
            })
    }
}

/// Every connection with current account names.
fn named_connections(conn: &rusqlite::Connection) -> Result<Vec<PartnerConnection>, PartnerError> {
    let mut connections = lis_interfaces::partner::list_partner_connections(conn)?;
    for connection in &mut connections {
        name_connection(conn, connection)?;
    }
    Ok(connections)
}

/// The connection with its history and current account names.
fn detail(
    conn: &rusqlite::Connection,
    id: PartnerConnectionId,
) -> Result<PartnerConnectionDetail, PartnerError> {
    let mut detail = lis_interfaces::partner::get_partner_connection(conn, id)?;
    name_connection(conn, &mut detail.connection)?;
    let names = display_names(
        conn,
        detail.history.iter().map(|entry| entry.changed.by.as_str()),
    )?;
    for entry in &mut detail.history {
        entry.changed.by_display = name_of(&names, &entry.changed.by);
    }
    Ok(detail)
}

fn name_connection(
    conn: &rusqlite::Connection,
    connection: &mut PartnerConnection,
) -> Result<(), PartnerError> {
    let verified = connection
        .verification
        .as_ref()
        .map(|check| check.verified.by.as_str());
    let names = display_names(
        conn,
        [connection.last_change.by.as_str()]
            .into_iter()
            .chain(verified),
    )?;
    connection.last_change.by_display = name_of(&names, &connection.last_change.by);
    if let Some(check) = &mut connection.verification {
        check.verified.by_display = name_of(&names, &check.verified.by);
    }
    Ok(())
}
