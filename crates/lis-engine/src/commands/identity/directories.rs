//! Same-identity directory maintenance and retained version history.
use super::MANAGE_CONFIGURATION;
use crate::{
    Engine,
    registry::{CommandSpec, spec},
};
use lis_core::CommandError;
use lis_identity::{
    ClientLocationView, ClientView, DirectoryHistoryInput, PractitionerView, UpdateClient,
    UpdateClientLocation, UpdatePractitioner,
};
pub(crate) const UPDATE_CLIENT: CommandSpec = spec(
    "update_client",
    "identity.update_client",
    MANAGE_CONFIGURATION,
);
pub(crate) const UPDATE_CLIENT_LOCATION: CommandSpec = spec(
    "update_client_location",
    "identity.update_client_location",
    MANAGE_CONFIGURATION,
);
pub(crate) const UPDATE_PRACTITIONER: CommandSpec = spec(
    "update_practitioner",
    "identity.update_practitioner",
    MANAGE_CONFIGURATION,
);
pub(crate) const DIRECTORY_HISTORY: CommandSpec = spec(
    "directory_history",
    "identity.directory_history",
    MANAGE_CONFIGURATION,
);
impl Engine {
    pub async fn update_client(&self, input: UpdateClient) -> Result<ClientView, CommandError> {
        self.run(&UPDATE_CLIENT, |call| async move {
            self.write(&call, move |ctx| lis_identity::update_client(ctx, input))
                .await
        })
        .await
    }
    pub async fn update_client_location(
        &self,
        input: UpdateClientLocation,
    ) -> Result<ClientLocationView, CommandError> {
        self.run(&UPDATE_CLIENT_LOCATION, |call| async move {
            self.write(&call, move |ctx| {
                lis_identity::update_client_location(ctx, input)
            })
            .await
        })
        .await
    }
    pub async fn update_practitioner(
        &self,
        input: UpdatePractitioner,
    ) -> Result<PractitionerView, CommandError> {
        self.run(&UPDATE_PRACTITIONER, |call| async move {
            self.write(&call, move |ctx| {
                lis_identity::update_practitioner(ctx, input)
            })
            .await
        })
        .await
    }
    pub async fn directory_history(
        &self,
        input: DirectoryHistoryInput,
    ) -> Result<Vec<lis_identity::DirectoryHistoryEntry>, CommandError> {
        self.run(&DIRECTORY_HISTORY, |call| async move {
            self.read(&call, move |conn| {
                lis_identity::directory_history(conn, input)
            })
            .await
        })
        .await
    }
}
