//! Attaching an accepted add-on component to the execution that already
//! measured it (result files brief F17, Q-41; REQ-ANA-001).

use lis_orders::OrderComponentId;
use lis_store::CommandContext;

use crate::error::{AnalyticalError, ensure_clear, invalid};
use crate::model::{ExecutionId, ExecutionState};
use crate::repo::published;
use crate::repo::runs as repo;
use crate::served::{CatalogCache, served_components};

/// Makes the active execution `execution_id` serve `order_component_id`
/// (append-only `ana_execution_components`), so a measurement it already
/// holds can become that component's result. Refused when the execution is
/// not active; when its method version does not serve the component for the
/// execution's accession; when the component is not active; when the gates
/// patient placement runs block: the container's specimen gate against the
/// method's specimen type (REQ-SPC-003, REQ-SPC-005) and the component's
/// order gate (REQ-ORD-002); or when an active execution already serves it.
/// The gates are read in the caller's transaction, also when the component's
/// item was already in the accession.
///
/// It checks no capability and appends no audit event: the caller's command
/// (`use_measured_result`) does both in the same transaction.
pub fn attach_served_component(
    ctx: &mut CommandContext<'_>,
    execution_id: ExecutionId,
    order_component_id: OrderComponentId,
) -> Result<(), AnalyticalError> {
    let execution =
        repo::execution(ctx.tx(), execution_id)?.ok_or(AnalyticalError::NotFound("execution"))?;
    if execution.view.state != ExecutionState::Active {
        return Err(AnalyticalError::WrongState("execution"));
    }
    let container = published::container(ctx.tx(), execution.view.container_id)?
        .ok_or(AnalyticalError::NotFound("container"))?;
    let (_, served) = served_components(
        ctx.tx(),
        &mut CatalogCache::default(),
        container.accession_id,
        execution.method_version_id,
    )?;
    let component = served
        .into_iter()
        .map(|served| served.component)
        .find(|component| component.component_id == order_component_id)
        .ok_or_else(|| {
            invalid(
                "orderComponentId",
                "not_served",
                "The execution's method does not perform this test for the specimen.",
            )
        })?;
    if !component.active {
        return Err(AnalyticalError::WrongState("order component"));
    }
    let method = lis_catalog::get_method_version(ctx.tx(), execution.method_version_id)?;
    let specimen =
        lis_specimens::testing_gate(ctx.tx(), container.container_id, &method.specimen_type_code)?;
    ensure_clear(specimen.err().unwrap_or_default())?;
    let order = lis_orders::testing_gate(ctx.tx(), order_component_id)?;
    ensure_clear(order.err().unwrap_or_default())?;
    if repo::component_in_progress(ctx.tx(), order_component_id)? {
        return Err(AnalyticalError::AlreadyServed);
    }
    repo::insert_execution_component(ctx.tx(), execution_id, order_component_id)?;
    Ok(())
}
