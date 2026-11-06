//! Listing, reading, adding, changing, starting, stopping and checking connections.

use lis_core::{CommandError, ErrorCode, FieldError};
use lis_interfaces::partner::{
    CreatePartnerConnection, MovedToChoice, PartnerConnection, PartnerConnectionDetail,
    PartnerConnectionId, PartnerConnectionSettings, PartnerError, RecordPartnerDestinationCheck,
    SetPartnerConnectionActive, UpdatePartnerConnection,
};
use lis_store::CommandContext;
use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;
use ts_rs::TS;

use super::contracts::name_contract;
use super::folder_choices::FolderChoiceId;
use super::{
    CREATE_PARTNER_CONNECTION, GET_PARTNER_CONNECTION, LIST_PARTNER_CONNECTIONS,
    RECORD_PARTNER_DESTINATION_CHECK, SET_PARTNER_CONNECTION_ACTIVE, UPDATE_PARTNER_CONNECTION,
};
use crate::commands::people::{display_names, name_of};
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
    /// For incoming orders over a folder: the Processed folder chosen the same way.
    #[serde(default)]
    #[ts(optional, type = "string")]
    pub processed_folder_choice: Option<FolderChoiceId>,
    /// For incoming orders over a folder: the Failed folder chosen the same way.
    #[serde(default)]
    #[ts(optional, type = "string")]
    pub failed_folder_choice: Option<FolderChoiceId>,
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
    /// A newly chosen Processed folder; absent keeps the current one.
    #[serde(default)]
    #[ts(optional, type = "string")]
    pub processed_folder_choice: Option<FolderChoiceId>,
    /// A newly chosen Failed folder; absent keeps the current one.
    #[serde(default)]
    #[ts(optional, type = "string")]
    pub failed_folder_choice: Option<FolderChoiceId>,
    /// A new SFTP password; absent keeps the current one for the same server and user.
    #[serde(default)]
    #[ts(optional, type = "string")]
    pub password: Option<SecretString>,
    pub reason: String,
}

impl CreatePartnerConnectionInput {
    /// The input, Processed and Failed folder handles.
    fn folder_choices(&self) -> [Option<FolderChoiceId>; 3] {
        [
            self.folder_choice,
            self.processed_folder_choice,
            self.failed_folder_choice,
        ]
    }
}

impl UpdatePartnerConnectionInput {
    /// The input, Processed and Failed folder handles.
    fn folder_choices(&self) -> [Option<FolderChoiceId>; 3] {
        [
            self.folder_choice,
            self.processed_folder_choice,
            self.failed_folder_choice,
        ]
    }
}

impl Engine {
    /// `list_partner_connections`: every connection by name, with how it is working.
    pub async fn list_partner_connections(&self) -> Result<Vec<PartnerConnection>, CommandError> {
        self.run(&LIST_PARTNER_CONNECTIONS, |call| async move {
            let mut connections = self.read(&call, named_connections).await?;
            for connection in &mut connections {
                self.partner_listeners.add_view(connection);
            }
            Ok(connections)
        })
        .await
    }

    /// `get_partner_connection`: one connection with its history.
    pub async fn get_partner_connection(
        &self,
        input: PartnerConnectionInput,
    ) -> Result<PartnerConnectionDetail, CommandError> {
        self.run(&GET_PARTNER_CONNECTION, |call| async move {
            self.connection_detail(&call, input.connection_id).await
        })
        .await
    }

    /// The connection as it is now, with its listener's state.
    pub(super) async fn connection_detail(
        &self,
        call: &Call,
        id: PartnerConnectionId,
    ) -> Result<PartnerConnectionDetail, Failure> {
        let detail = self.read(call, move |conn| detail(conn, id)).await?;
        Ok(self.with_listener(detail, false).await)
    }

    /// The connection with its listener's state; after a change, once the listeners follow
    /// it.
    async fn with_listener(
        &self,
        mut detail: PartnerConnectionDetail,
        changed: bool,
    ) -> PartnerConnectionDetail {
        if changed {
            self.listen_for_partners().await;
        }
        self.partner_listeners.add_view(&mut detail.connection);
        detail
    }

    /// `create_partner_connection`: started, or stopped with `active` false. It writes no
    /// patient data until its destination check is recorded.
    pub async fn create_partner_connection(
        &self,
        input: CreatePartnerConnectionInput,
    ) -> Result<PartnerConnectionDetail, CommandError> {
        self.run(&CREATE_PARTNER_CONNECTION, |call| async move {
            let folders = input.folder_choices();
            let password = input.password.as_ref();
            let chosen = self
                .chosen(&call, &input.settings, folders, password)
                .await?;
            self.change(&call, move |ctx| {
                lis_interfaces::partner::create_partner_connection(
                    ctx,
                    CreatePartnerConnection {
                        settings: input.settings,
                        folder: chosen.folder.unwrap_or_default(),
                        moved_to: chosen.moved_to,
                        sealed_password: chosen.sealed_password,
                        active: input.active,
                    },
                )
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
            let folders = input.folder_choices();
            let password = input.password.as_ref();
            let chosen = self
                .chosen(&call, &input.settings, folders, password)
                .await?;
            self.change(&call, move |ctx| {
                lis_interfaces::partner::update_partner_connection(
                    ctx,
                    UpdatePartnerConnection {
                        connection_id: input.connection_id,
                        expected_revision: input.expected_revision,
                        settings: input.settings,
                        folder: chosen.folder,
                        moved_to: chosen.moved_to,
                        sealed_password: chosen.sealed_password,
                        reason: input.reason,
                    },
                )
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
            self.change(&call, move |ctx| {
                lis_interfaces::partner::set_partner_connection_active(ctx, input)
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
            self.change(&call, move |ctx| {
                lis_interfaces::partner::record_partner_destination_check(ctx, input)
            })
            .await
        })
        .await
    }

    /// Writes a change to a connection, then returns it once the listeners follow it, so a
    /// saved or started listener shows at once whether it listens.
    pub(super) async fn change(
        &self,
        call: &Call,
        change: impl FnOnce(&mut CommandContext<'_>) -> Result<PartnerConnectionId, PartnerError>
        + Send
        + 'static,
    ) -> Result<PartnerConnectionDetail, Failure> {
        let detail = self
            .write(call, move |ctx| {
                let id = change(ctx)?;
                detail(ctx.tx(), id)
            })
            .await?;
        Ok(self.with_listener(detail, true).await)
    }

    /// Authorizes a create or update, checks an incoming connection's client, and resolves
    /// the chosen input, Processed and Failed folders and the password.
    async fn chosen(
        &self,
        call: &Call,
        settings: &PartnerConnectionSettings,
        [folder, processed, failed]: [Option<FolderChoiceId>; 3],
        password: Option<&SecretString>,
    ) -> Result<Chosen, Failure> {
        self.authorize(call).await?;
        self.ordering_client(call, settings).await?;
        Ok(Chosen {
            folder: self.chosen_folder(call, folder)?,
            moved_to: self.chosen_moved_to(call, processed, failed)?,
            sealed_password: self.sealed_password(password).await?,
        })
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

    /// An incoming connection's partner is the client its orders are for: one of the
    /// laboratory's active clients, by code.
    async fn ordering_client(
        &self,
        call: &Call,
        settings: &PartnerConnectionSettings,
    ) -> Result<(), Failure> {
        if !settings.message_type.incoming() {
            return Ok(());
        }
        let clients = self.read(call, lis_identity::list_active_clients).await?;
        let code = settings.partner.trim();
        if clients.iter().any(|client| client.code == code) {
            Ok(())
        } else {
            Err(refused(
                "settings.partner",
                "unknown_client",
                "Choose one of the laboratory's active clients.",
            ))
        }
    }
}

/// What a create or update keeps besides its settings, resolved before the write.
struct Chosen {
    folder: Option<String>,
    moved_to: MovedToChoice,
    sealed_password: Option<Vec<u8>>,
}

/// A refusal of one form field.
pub(super) fn refused(field: &str, code: &str, message: &str) -> Failure {
    Failure::Refused {
        code: ErrorCode::Validation,
        message: message.to_owned(),
        field_errors: vec![FieldError {
            field: field.to_owned(),
            code: code.to_owned(),
            message: message.to_owned(),
        }],
        gate_reasons: Vec::new(),
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
    name_contract(conn, &mut detail.contract)?;
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
