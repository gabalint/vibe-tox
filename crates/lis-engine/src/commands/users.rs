//! User administration (REQ-IDN-004). Accounts, non-clinical roles and resets
//! need `manage_users`; clinical roles need `manage_quality`, so a quality
//! manager also reads the users screen. `lis-auth` checks each change, the
//! actor's password and the refusal of administering one's own account.

use lis_auth::{
    ChangeUserStatusInput, CreateUserInput, DesignateOnePersonLaboratoryInput,
    ResetUserCredentialsInput, RoleInfo, SetUserRolesInput, UserSummary,
};
use lis_core::{Capability, CommandError};

use crate::engine::Engine;
use crate::pipeline::{Call, Failure, accepted};
use crate::registry::{
    CREATE_USER, DESIGNATE_ONE_PERSON_LABORATORY, DISABLE_USER, ENABLE_USER, LIST_ROLES,
    LIST_USERS, RESET_USER_CREDENTIALS, SET_USER_ROLES,
};

impl Engine {
    /// The users screen's reads: `manage_users`, or `manage_quality` for the
    /// quality manager who assigns clinical roles. A denial names `manage_users`.
    async fn authorize_users_screen(&self, call: &Call) -> Result<(), Failure> {
        if call.principal.require(Capability::ManageQuality).is_ok() {
            return Ok(());
        }
        self.authorize(call).await
    }

    /// `list_roles`: the fixed role catalog, with the roles the caller may assign.
    pub async fn list_roles(&self) -> Result<Vec<RoleInfo>, CommandError> {
        self.run(&LIST_ROLES, |call| async move {
            self.authorize_users_screen(&call).await?;
            let caller = call.principal.clone();
            self.db
                .read(move |conn| lis_auth::list_roles(conn, &caller))
                .await
                .map_err(Failure::from)
        })
        .await
    }

    /// `list_users`.
    pub async fn list_users(&self) -> Result<Vec<UserSummary>, CommandError> {
        self.run(&LIST_USERS, |call| async move {
            self.authorize_users_screen(&call).await?;
            let now = self.clock.now();
            self.db
                .read(move |conn| lis_auth::list_users(conn, now))
                .await
                .map_err(Failure::from)
        })
        .await
    }

    /// `create_user` (the administrator's password again).
    pub async fn create_user(&self, input: CreateUserInput) -> Result<UserSummary, CommandError> {
        self.run(&CREATE_USER, |call| async move {
            let attempt = self
                .write(&call, move |ctx| lis_auth::create_user(ctx, input))
                .await?;
            accepted(attempt)
        })
        .await
    }

    /// `set_user_roles` (the actor's password again).
    pub async fn set_user_roles(
        &self,
        input: SetUserRolesInput,
    ) -> Result<UserSummary, CommandError> {
        self.run(&SET_USER_ROLES, |call| async move {
            let attempt = self
                .write(&call, move |ctx| lis_auth::set_user_roles(ctx, input))
                .await?;
            accepted(attempt)
        })
        .await
    }

    /// `designate_one_person_laboratory` (the actor's password again; ADR 0014): the one
    /// active account gets every role, and each duty may be done alone.
    pub async fn designate_one_person_laboratory(
        &self,
        input: DesignateOnePersonLaboratoryInput,
    ) -> Result<UserSummary, CommandError> {
        self.run(&DESIGNATE_ONE_PERSON_LABORATORY, |call| async move {
            let attempt = self
                .write(&call, move |ctx| {
                    lis_auth::designate_one_person_laboratory(ctx, input)
                })
                .await?;
            accepted(attempt)
        })
        .await
    }

    /// `disable_user`.
    pub async fn disable_user(
        &self,
        input: ChangeUserStatusInput,
    ) -> Result<UserSummary, CommandError> {
        self.run(&DISABLE_USER, |call| async move {
            self.write(&call, move |ctx| lis_auth::disable_user(ctx, input))
                .await
        })
        .await
    }

    /// `enable_user`.
    pub async fn enable_user(
        &self,
        input: ChangeUserStatusInput,
    ) -> Result<UserSummary, CommandError> {
        self.run(&ENABLE_USER, |call| async move {
            self.write(&call, move |ctx| lis_auth::enable_user(ctx, input))
                .await
        })
        .await
    }

    /// `reset_user_credentials` (the administrator's password again). The
    /// reset ends the user's sessions; a sign-in in progress in this window is
    /// discarded too, so it cannot finish on the old credentials.
    pub async fn reset_user_credentials(
        &self,
        input: ResetUserCredentialsInput,
    ) -> Result<UserSummary, CommandError> {
        self.run(&RESET_USER_CREDENTIALS, |call| async move {
            let attempt = self
                .write(&call, move |ctx| {
                    lis_auth::reset_user_credentials(ctx, input)
                })
                .await?;
            let user = accepted(attempt)?;
            self.challenges.clear();
            Ok(user)
        })
        .await
    }
}
