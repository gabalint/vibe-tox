//! Software authenticator for synthetic engine tests. This deliberately
//! simulates user verification and does not qualify physical security keys.

use lis_core::CommandError;
use lis_engine::SecurityKeyClient;
use serde_json::Value;
use std::sync::Mutex;
use webauthn_authenticator_rs::{WebauthnAuthenticator, softpasskey::SoftPasskey};
use webauthn_rs::prelude::Url;

pub(crate) struct TestKey {
    authenticator: Mutex<WebauthnAuthenticator<SoftPasskey>>,
    user_verified: bool,
}

impl TestKey {
    pub(crate) fn new() -> Self {
        Self::with_user_verification(true)
    }

    pub(crate) fn with_user_verification(user_verified: bool) -> Self {
        Self {
            authenticator: Mutex::new(WebauthnAuthenticator::new(SoftPasskey::new(true))),
            user_verified,
        }
    }
}

impl SecurityKeyClient for TestKey {
    async fn register(&self, origin: String, mut options: Value) -> Result<Value, CommandError> {
        // A weaker synthetic client can ignore Required. The engine must reject its signed UV=0.
        if !self.user_verified {
            options["publicKey"]["authenticatorSelection"]["userVerification"] =
                Value::String("discouraged".to_owned());
        }
        let response = self
            .authenticator
            .lock()
            .unwrap()
            .do_registration(
                Url::parse(&origin).unwrap(),
                serde_json::from_value(options).unwrap(),
            )
            .unwrap();
        Ok(serde_json::to_value(response).unwrap())
    }

    async fn authenticate(
        &self,
        origin: String,
        mut options: Value,
    ) -> Result<Value, CommandError> {
        if !self.user_verified {
            options["publicKey"]["userVerification"] = Value::String("discouraged".to_owned());
        }
        let response = self
            .authenticator
            .lock()
            .unwrap()
            .do_authentication(
                Url::parse(&origin).unwrap(),
                serde_json::from_value(options).unwrap(),
            )
            .unwrap();
        Ok(serde_json::to_value(response).unwrap())
    }
}
