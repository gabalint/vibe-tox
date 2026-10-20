use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use webauthn_authenticator_rs::{
    error::WebauthnCError, transport::Token, ui::UiCallback, usb::USBToken,
};
use webauthn_rs_proto::AuthenticatorTransport;

use crate::interaction::InteractionState;

/// The upstream CTAP adapter ignores its timeout argument. Enforce our deadline
/// on the real transport, and cancel the device rather than abandoning a read.
pub(crate) struct BoundedUsbToken {
    inner: USBToken,
    shared: Arc<InteractionState>,
}

impl std::fmt::Debug for BoundedUsbToken {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("BoundedUsbToken")
    }
}

impl BoundedUsbToken {
    pub(crate) fn new(inner: USBToken, shared: Arc<InteractionState>) -> Self {
        Self { inner, shared }
    }
}

#[async_trait]
impl Token for BoundedUsbToken {
    type Id = <USBToken as Token>::Id;

    fn get_transport(&self) -> AuthenticatorTransport {
        AuthenticatorTransport::Usb
    }

    async fn transmit_raw<U>(&mut self, bytes: &[u8], ui: &U) -> Result<Vec<u8>, WebauthnCError>
    where
        U: UiCallback,
    {
        let result = tokio::select! {
            biased;
            _ = self.shared.stopped() => Err(WebauthnCError::Cancelled),
            result = self.inner.transmit_raw(bytes, ui) => result,
        };
        if self.shared.check().is_err() {
            let _ = tokio::time::timeout(Duration::from_millis(250), self.inner.cancel()).await;
            return Err(WebauthnCError::Cancelled);
        }
        result
    }

    async fn init(&mut self) -> Result<(), WebauthnCError> {
        tokio::select! {
            biased;
            _ = self.shared.stopped() => Err(WebauthnCError::Cancelled),
            result = self.inner.init() => result,
        }
    }

    async fn cancel(&mut self) -> Result<(), WebauthnCError> {
        self.inner.cancel().await
    }

    async fn close(&mut self) -> Result<(), WebauthnCError> {
        self.inner.close().await
    }
}
