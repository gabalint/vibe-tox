//! The trusted native adapter; the engine verifies every returned assertion.

use lis_core::CommandError;
use lis_engine::SecurityKeyClient;
use lis_security_key::SecurityKeyError;
use serde_json::Value;

use super::{Operation, native_error};

impl SecurityKeyClient for Operation<'_> {
    fn finish(&self) -> Result<(), CommandError> {
        self.seal()
    }

    async fn register(&self, origin: String, options: Value) -> Result<Value, CommandError> {
        self.await_replacement().await?;
        let interaction = self.start_native()?;
        let response = lis_security_key::register(&origin, options, interaction).await;
        self.end_native()?;
        serde_json::to_value(response.map_err(native_error)?)
            .map_err(|_| native_error(SecurityKeyError::InvalidResponse))
    }

    async fn authenticate(&self, origin: String, options: Value) -> Result<Value, CommandError> {
        let interaction = self.start_native()?;
        let response = lis_security_key::authenticate(&origin, options, interaction).await;
        self.end_native()?;
        let response = response.map_err(native_error)?;
        self.authenticated()?;
        serde_json::to_value(response).map_err(|_| native_error(SecurityKeyError::InvalidResponse))
    }
}
