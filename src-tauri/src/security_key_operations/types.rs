//! Public progress and secret-aware control inputs.

use lis_security_key::KeyStatus;
use secrecy::SecretString;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Clone, Copy, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub(super) enum Progress {
    WaitingForTouch,
    PinRequired,
    Processing,
    WaitingForReplacement,
}

impl From<KeyStatus> for Progress {
    fn from(status: KeyStatus) -> Self {
        match status {
            KeyStatus::WaitingForTouch => Self::WaitingForTouch,
            KeyStatus::PinRequired => Self::PinRequired,
            KeyStatus::Processing => Self::Processing,
        }
    }
}

#[derive(Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub(crate) struct SecurityKeyStatus {
    pub(super) operation_id: String,
    #[ts(type = "'waitingForTouch' | 'pinRequired' | 'processing' | 'waitingForReplacement'")]
    pub(super) status: Progress,
}

#[derive(Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub(crate) struct SecurityKeyOperationInput {
    pub(super) operation_id: String,
}

#[derive(Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub(crate) struct SecurityKeyPinInput {
    pub(super) operation_id: String,
    #[ts(type = "string")]
    pub(super) pin: SecretString,
}
