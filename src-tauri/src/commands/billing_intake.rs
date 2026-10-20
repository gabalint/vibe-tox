//! Patient and order financial intake through the authorized engine pipeline.

use lis_core::CommandError;
use lis_engine::{
    BillingReviewRecord, Engine, GetOrderInput, GetPatientBillingProfileInput, OrderBillingContext,
    OrderBillingView, PatientBillingProfile, PatientBillingProfileView,
    SetOrderBillingContextInput, SetPatientBillingProfileInput, UpdateBillingReviewItemInput,
};
use tauri::State;

#[tauri::command]
pub(crate) async fn get_patient_billing_profile(
    engine: State<'_, Engine>,
    input: GetPatientBillingProfileInput,
) -> Result<PatientBillingProfileView, CommandError> {
    engine.get_patient_billing_profile(input).await
}

#[tauri::command]
pub(crate) async fn set_patient_billing_profile(
    engine: State<'_, Engine>,
    input: SetPatientBillingProfileInput,
) -> Result<PatientBillingProfile, CommandError> {
    engine.set_patient_billing_profile(input).await
}

#[tauri::command]
pub(crate) async fn get_order_billing(
    engine: State<'_, Engine>,
    input: GetOrderInput,
) -> Result<OrderBillingView, CommandError> {
    engine.get_order_billing(input).await
}

#[tauri::command]
pub(crate) async fn set_order_billing_context(
    engine: State<'_, Engine>,
    input: SetOrderBillingContextInput,
) -> Result<OrderBillingContext, CommandError> {
    engine.set_order_billing_context(input).await
}

#[tauri::command]
pub(crate) async fn update_billing_review_item(
    engine: State<'_, Engine>,
    input: UpdateBillingReviewItemInput,
) -> Result<BillingReviewRecord, CommandError> {
    engine.update_billing_review_item(input).await
}

#[tauri::command]
pub(crate) async fn list_billing_review_items(
    engine: State<'_, Engine>,
    input: lis_engine::ListBillingReviewItemsInput,
) -> Result<Vec<lis_engine::BillingReviewRecord>, CommandError> {
    engine.list_billing_review_items(input).await
}
