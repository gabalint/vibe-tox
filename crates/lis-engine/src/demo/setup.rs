//! First-run demo preparation after normal sign-in (REQ-OPS-014, REQ-OPS-016).

use lis_catalog::{LoadStandardCatalog, StandardFile};
use lis_core::Capability;
use lis_ops::InstallationKind;
use lis_store::CommandContext;

use super::{DemoError, load};

/// Catalog drafts, synthetic records and completion commit together. No clinical
/// configuration is approved and the authenticated first account authors every draft.
pub(crate) fn complete_setup(
    ctx: &mut CommandContext<'_>,
    kind: InstallationKind,
) -> Result<(), DemoError> {
    ctx.require(Capability::ManageConfiguration)?;
    if kind != InstallationKind::Demo {
        return Err(DemoError::NotDemo);
    }
    if !lis_auth::demo_setup_pending_for(ctx)? {
        return Ok(());
    }
    let file = StandardFile::embedded()?;
    let tree = lis_catalog::standard_catalog_tree(ctx.tx(), &file)?;
    lis_catalog::load_standard_catalog(
        ctx,
        &file,
        LoadStandardCatalog {
            catalog_version: tree.info.catalog_version,
            orderable_codes: tree.orderables.into_iter().map(|item| item.code).collect(),
            method_codes: tree.methods.into_iter().map(|item| item.code).collect(),
            omitted_analytes: Vec::new(),
            // The setup choice explicitly acknowledges draft catalog content and synthetic data.
            disclaimer_acknowledged: true,
        },
    )?;
    load(ctx, kind)?;
    lis_auth::complete_demo_setup(ctx)?;
    Ok(())
}
