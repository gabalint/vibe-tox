//! Setup, sign-in and session commands (REQ-IDN-004, NFR-19).

use lis_auth::{
    Attempt, ChangeInitialPasswordInput, ChangePasswordInput, CreateFirstAdministratorInput,
    EndReason, LockTrigger, NewSession, Rejection, SessionUser, SetupStatus, SignInInput,
    SignInStep, UnlockSessionInput, VerifyMfaInput,
};
use lis_core::{Actor, CommandError, ErrorCode, Principal, SessionId, UserId};
use lis_ops::InstallationKind;

use crate::engine::Engine;
use crate::pipeline::{Failure, accepted};
use crate::registry::{
    CHANGE_INITIAL_PASSWORD, CHANGE_PASSWORD, CREATE_FIRST_ADMINISTRATOR, CURRENT_SESSION,
    LOCK_SESSION, SETUP_STATUS, SIGN_IN, SIGN_OUT, TOUCH_SESSION, UNLOCK_SESSION, VERIFY_MFA,
};
use crate::session::{Session, SessionInfo, SessionState};

/// The session behind a user principal.
pub(super) fn session_of(principal: &Principal) -> Result<(UserId, SessionId), Failure> {
    match principal.actor() {
        Actor::User {
            user_id,
            session_id,
            ..
        } => Ok((*user_id, *session_id)),
        Actor::Instrument { .. } | Actor::System { .. } => Err(Failure::Internal(
            "session command without a user".to_owned(),
        )),
    }
}

impl Engine {
    /// `setup_status` (public).
    pub async fn setup_status(&self) -> Result<SetupStatus, CommandError> {
        self.run(&SETUP_STATUS, |call| async move {
            self.read(&call, lis_auth::setup_status).await
        })
        .await
    }

    /// `create_first_administrator` (public; refused once any user exists).
    pub async fn create_first_administrator(
        &self,
        input: CreateFirstAdministratorInput,
    ) -> Result<(), CommandError> {
        self.run(&CREATE_FIRST_ADMINISTRATOR, |call| async move {
            if input.load_demo_data.unwrap_or(false)
                && self.installation_kind != InstallationKind::Demo
            {
                return Err(Failure::refused(
                    ErrorCode::Conflict,
                    "Demo data can be prepared only in the separate demo laboratory.",
                ));
            }
            self.write(&call, move |ctx| {
                lis_auth::create_first_administrator(ctx, input)
            })
            .await
            .map(drop)
        })
        .await
    }

    /// `sign_in` (public): the password step.
    pub async fn sign_in(&self, input: SignInInput) -> Result<SignInStep, CommandError> {
        self.run(&SIGN_IN, |call| async move {
            let attempt = self
                .write(&call, move |ctx| lis_auth::check_password(ctx, &input))
                .await?;
            let password = accepted(attempt).inspect_err(|_| self.challenges.clear())?;
            Ok(self.challenges.issue(&password, self.clock.now())?)
        })
        .await
    }

    /// `change_initial_password` (public; needs a live challenge).
    pub async fn change_initial_password(
        &self,
        input: ChangeInitialPasswordInput,
    ) -> Result<SignInStep, CommandError> {
        self.run(&CHANGE_INITIAL_PASSWORD, |call| async move {
            let challenge = self.challenges.live(input.challenge_id, self.clock.now())?;
            let password = self
                .write(&call, move |ctx| {
                    lis_auth::set_initial_password(ctx, &challenge, input.new_password)
                })
                .await?;
            Ok(self.challenges.issue(&password, self.clock.now())?)
        })
        .await
    }

    /// `verify_mfa` (public; needs a live challenge): confirms a pending
    /// enrolment and starts the session, replacing any previous one.
    pub async fn verify_mfa(&self, input: VerifyMfaInput) -> Result<SessionInfo, CommandError> {
        self.run(&VERIFY_MFA, |call| async move {
            let challenge = self.challenges.live(input.challenge_id, self.clock.now())?;
            let key = self.data_key().await?;
            let session = NewSession {
                id: SessionId::generate(),
                workstation_id: self.workstation_id,
                webview: self.webview.clone(),
                replaces: self.open_session().map(|session| session.id),
            };
            let session_id = session.id;
            let attempt = self
                .write(&call, move |ctx| {
                    lis_auth::complete_sign_in(ctx, &key, &challenge, &input.code, &session)
                })
                .await?;
            // A wrong code may be retried; a lockout ends the sign-in.
            if matches!(attempt, Attempt::Rejected(Rejection::LockedOut)) {
                self.challenges.clear();
            }
            let user = accepted(attempt)?;
            self.challenges.complete(input.challenge_id);
            self.activate(session_id, &user);
            Ok(SessionInfo::new(&user, false, self.security_policy()))
        })
        .await
    }

    /// `current_session` (public): the session, after any idle lock.
    pub async fn current_session(&self) -> Result<Option<SessionInfo>, CommandError> {
        self.run(&CURRENT_SESSION, |call| async move {
            let Some((session, locked)) = self.observe_session(call.correlation_id).await? else {
                return Ok(None);
            };
            let user_id = session.user_id;
            let user: SessionUser = self
                .db
                .read(move |conn| lis_auth::session_user(conn, user_id))
                .await
                .map_err(Failure::from)?;
            Ok(Some(SessionInfo::new(
                &user,
                locked,
                self.security_policy(),
            )))
        })
        .await
    }

    /// `touch_session`: user activity; the pipeline already recorded it.
    pub async fn touch_session(&self) -> Result<(), CommandError> {
        self.run(&TOUCH_SESSION, |_| async { Ok(()) }).await
    }

    /// `lock_session` (also when locked).
    pub async fn lock_session(&self) -> Result<(), CommandError> {
        self.run(&LOCK_SESSION, |call| async move {
            match self.lock_if_active() {
                Some(session) => {
                    self.audit_lock(session, LockTrigger::User, call.correlation_id)
                        .await
                }
                None => Ok(()),
            }
        })
        .await
    }

    /// `unlock_session` (locked session; the same user's password).
    pub async fn unlock_session(
        &self,
        input: UnlockSessionInput,
    ) -> Result<SessionInfo, CommandError> {
        self.run(&UNLOCK_SESSION, |call| async move {
            let _gate = self.key_ceremonies.enter()?;
            let guard = self.key_ceremonies.guard(self.clock.now());
            let (_, session_id) = session_of(&call.principal)?;
            let attempt = self
                .write(&call, move |ctx| lis_auth::unlock(ctx, &input.password))
                .await?;
            let user = accepted(attempt)?;
            self.finish_authenticated_session(&call, &guard, session_id, &user, false)
                .await
        })
        .await
    }

    /// `sign_out` (also when locked).
    pub async fn sign_out(&self) -> Result<(), CommandError> {
        self.run(&SIGN_OUT, |call| async move {
            let (_, session_id) = session_of(&call.principal)?;
            // Cancel pending unlocks before waiting for the database writer.
            self.forget_session(session_id);
            self.challenges.clear();
            self.write(&call, move |ctx| {
                lis_auth::end_session(ctx, session_id, EndReason::SignedOut)
            })
            .await
        })
        .await
    }

    /// `change_password` (unlocked session).
    pub async fn change_password(&self, input: ChangePasswordInput) -> Result<(), CommandError> {
        self.run(&CHANGE_PASSWORD, |call| async move {
            let attempt = self
                .write(&call, move |ctx| lis_auth::change_password(ctx, input))
                .await?;
            accepted(attempt)
        })
        .await
    }

    /// The window's session, locked or not.
    pub(crate) fn open_session(&self) -> Option<Session> {
        match *self.session_state() {
            SessionState::None => None,
            SessionState::Active(session) | SessionState::Locked(session) => Some(session),
        }
    }

    /// Makes `session_id` of `user` the window's unlocked session.
    pub(crate) fn activate(&self, session_id: SessionId, user: &SessionUser) {
        let mut state = self.session_state();
        self.set_active(&mut state, session_id, user);
    }

    /// The final generation check and activation share the session-state lock with
    /// lock/signout. A delayed database write cannot overwrite either transition.
    pub(crate) fn activate_if_current(
        &self,
        session_id: SessionId,
        user: &SessionUser,
        generation: u64,
    ) -> bool {
        let mut state = self.session_state();
        if self
            .key_ceremonies
            .generation
            .load(std::sync::atomic::Ordering::SeqCst)
            != generation
        {
            return false;
        }
        self.set_active(&mut state, session_id, user);
        true
    }

    fn set_active(&self, state: &mut SessionState, session_id: SessionId, user: &SessionUser) {
        *state = SessionState::Active(Session {
            id: session_id,
            user_id: user.user_id,
            last_activity: self.clock.now(),
        });
        self.key_ceremonies.invalidate();
    }
}
