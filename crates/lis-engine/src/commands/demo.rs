//! The demo laboratory's commands (proposed REQ-OPS-016): its status, and loading the
//! shipped demo data. Switching between the laboratory and the demo laboratory, and
//! resetting the demo, are desktop-shell steps that restart the app.

use lis_core::{Capability, CommandError};
use lis_ops::InstallationKind;
use serde::Serialize;
use ts_rs::TS;

use crate::demo::{DemoDataLoaded, complete_setup, demo_data_loaded, load};
use crate::engine::Engine;
use crate::registry::{Access, CommandSpec, spec};

pub(crate) const DEMO_STATUS: CommandSpec = spec("demo_status", "demo.status", Access::Session);
/// The loader also needs `enter_orders` to register the patients; the domain commands check
/// each capability.
pub(crate) const LOAD_DEMO_DATA: CommandSpec = spec(
    "load_demo_data",
    crate::demo::LOAD_DEMO_DATA,
    Access::Capability(Capability::ManageConfiguration),
);
pub(crate) const COMPLETE_DEMO_SETUP: CommandSpec = spec(
    "complete_demo_setup",
    "demo.complete_setup",
    Access::Capability(Capability::ManageConfiguration),
);

/// `demo_status()`: whether this is the demo laboratory and whether its demo data is loaded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DemoStatus {
    /// The installation is the demo laboratory.
    pub is_demo: bool,
    /// The demo file's clients, practitioners and patients are loaded; always false in a
    /// laboratory installation.
    pub demo_data_loaded: bool,
}

impl Engine {
    /// Completes the first account's requested demo preparation after normal MFA sign-in.
    /// Loading and the durable completion marker share one transaction; retry is safe.
    pub async fn complete_demo_setup(&self) -> Result<(), CommandError> {
        self.run(&COMPLETE_DEMO_SETUP, |call| async move {
            let kind = self.installation_kind;
            self.write(&call, move |ctx| complete_setup(ctx, kind))
                .await
        })
        .await
    }

    /// `demo_status` (any signed-in session): no patient data, only two facts.
    pub async fn demo_status(&self) -> Result<DemoStatus, CommandError> {
        self.run(&DEMO_STATUS, |call| async move {
            let is_demo = self.installation_kind == InstallationKind::Demo;
            let demo_data_loaded = if is_demo {
                self.read(&call, demo_data_loaded).await?
            } else {
                false
            };
            Ok(DemoStatus {
                is_demo,
                demo_data_loaded,
            })
        })
        .await
    }

    /// `load_demo_data`: refused outside the demo laboratory; loading again creates nothing
    /// new and fills demo cutoffs the caller's drafts still leave blank.
    pub async fn load_demo_data(&self) -> Result<DemoDataLoaded, CommandError> {
        self.run(&LOAD_DEMO_DATA, |call| async move {
            let kind = self.installation_kind;
            self.write(&call, move |ctx| load(ctx, kind)).await
        })
        .await
    }
}
