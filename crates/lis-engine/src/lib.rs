//! Engine composition root (REQ-IDN-003, REQ-IDN-004, REQ-OPS-001, NFR-08,
//! NFR-19, NFR-31): opens the database with every module schema, runs startup
//! reconciliation, owns the window's session, and runs every IPC command
//! through one pipeline (`.ai/skills/engine-command/SKILL.md`):
//!
//! 1. **Caller.** Public commands run as the `sign-in-screen` system
//!    principal. Every other command needs a session: none gives
//!    `unauthenticated`; a locked session, or one idle past [`IDLE_TIMEOUT`]
//!    (which this locks and audits), gives `session_locked`. The principal is
//!    the session's user with the capabilities their roles grant *now*.
//! 2. **Authorize.** Commands call `ctx.require` in their domain function;
//!    queries are checked against the [`registry`] entry. A denial rolls back
//!    and is audited as a `security` event with outcome `denied` in its own
//!    transaction.
//! 3. **Run.** Writes run on the single writer in `BEGIN IMMEDIATE`; reads on
//!    read connections; sensitive reads append an `access` event before
//!    returning data and fail if it cannot be written.
//! 4. **Finish.** Exactly one completion log line per command (command,
//!    action, correlation ID, principal ID, outcome, error code, duration),
//!    and failures become [`CommandError`](lis_core::CommandError)s with the
//!    correlation ID; internal errors get a generic message.
//!
//! [`registry::COMMANDS`] lists every IPC command with its action and access;
//! the table-driven authorization test runs over it.
//!
//! Startup: open the database, reconcile (register the workstation, end
//! sessions a crash left open), start the outbox dispatcher. Shutdown runs in
//! reverse. Client and site scoping of commands (REQ-IDN-003) comes with the
//! first scoped records.

mod commands;
mod engine;
mod keys;
mod outbox_worker;
mod pipeline;
pub mod registry;
mod session;
pub use commands::security_keys::{
    KeyRecoveryAcknowledgement, KeyRecoveryResult, KeySignInChoicesInput, RevokeSecurityKeyInput,
    SecurityKeyAction, SecurityKeyActionInput, SecurityKeyClient, SecurityKeyEnrollInput,
    SecurityKeyRecoveryInput, SecurityKeySignInInput, SecurityKeySignupInput,
    SecurityKeyUnlockInput,
};

pub use commands::audit::{
    AuditEventView, AuditStream, ChainFinding, ChainVerification, ListAuditEventsInput,
    VerifyAuditChainInput,
};
pub use commands::listeners::ListenerStatusView;
// Wave 3+: re-exports per area. Edit only your block.
// identity: begin
pub use commands::identity::{GetSubjectInput, ListIdentityHoldsInput};
pub use lis_identity::{
    AddClientLocation, AddSubjectIdentifier, AssertDemographics, ClientId, ClientLocationId,
    ClientView, ConfirmDistinctSubjects, CreateClient, DeactivateSubjectIdentifier,
    IdentifierAdded, IdentityHoldResolved, IdentityHoldView, LaboratoryProfileView, PractitionerId,
    PractitionerView, RegisterPractitioner, RegisterSubject, SetLaboratoryProfile,
    SubjectCandidates, SubjectMatchQuery, SubjectRegistered, SubjectRevision, SubjectSearch,
    SubjectSummary, SubjectView,
};
// identity: end
// catalog: begin
pub use commands::catalog::preview::CatalogPreviewPdfInput;
pub use commands::catalog::standard::ApproveReviewedCatalogInput;
pub use commands::catalog::versions::{
    GetMethodVersionInput, GetOrderableVersionInput, ListMethodVersionsInput,
    ListOrderableVersionsInput,
};
pub use commands::catalog::{GetValidityPolicyInput, ListValidityPolicyVersionsInput};
pub use lis_catalog::{
    AnalyteId, AnalyteView, ApproveMethodVersion, ApproveOrderableVersion,
    ApproveValidityPolicyVersion, CodedName, CreateAnalyte, CreateOrderable, DraftMethodVersion,
    DraftOrderableVersion, DraftValidityPolicyVersion, MethodId, MethodVersion, MethodVersionView,
    MethodView, OrderableId, OrderableVersion, OrderableVersionView, OrderableView,
    RetireMethodVersion, RetireOrderableVersion, RetireValidityPolicyVersion, SpecimenTypeId,
    SpecimenTypeView, UpdateMethodDraft, UpdateOrderableDraft, UpdateValidityPolicyDraft,
    ValidityPolicyVersion, ValidityPolicyView, VersionRef, VersionSummary,
};
pub use lis_catalog::{
    CatalogBlankCutoff, CatalogPreviewBlankCutoff, CatalogPreviewId, CatalogPreviewSummary,
    CatalogReviewApproved, LoadStandardCatalog, StandardCatalogLoaded, StandardCatalogStatus,
    StandardCatalogTree, StandardOmittedAnalyte,
};
// catalog: end
// interfaces: begin
pub use commands::interfaces::InstrumentChangeResult;
pub use lis_interfaces::{
    Instrument, RegisterInstrument, SetInstrumentEnabled, UpdateInstrumentConnection,
};
// interfaces: end
// orders: begin
pub use lis_orders::{
    AcceptOrderInput, AmendOrderEvidenceInput, AuthorityEvidenceInput, AuthorityEvidenceKind,
    BillingResponsibility, BillingReviewElement, CancellationView, ClarificationDeficiency,
    ClarificationHoldView, CreateBillingReviewItemInput, DecideCancellationInput,
    DecideFollowOnInput, Decision, DiagnosisInput, EvidenceInput, GetOrderInput, ItemKind,
    ListOrdersInput, ListStandingOrdersInput, MedicationInput, MedicationSource, OrderDetail,
    OrderEvidenceId, OrderFilter, OrderId, OrderItemId, OrderItemInput, OrderStatus, OrderSummary,
    PlaceClarificationHoldInput, PointOfCareResultInput, RecordTriggerNotMetInput,
    ReflexAuthorization, RegisterOrderInput, RegisterStandingOrderInput, RejectOrderInput,
    RequestCancellationInput, RequestFollowOnInput, ResolveClarificationHoldInput, SequenceRole,
    SourceKind, StandingOrderId, StandingOrderView, TemplateUseInput, TriggerSource,
};
// orders: end
// specimens: begin
pub use lis_specimens::ResolveReceiptChronologyInput;
mod specimen_labels;
pub use commands::specimens::{FindContainerInput, GetAccessionInput};
pub use lis_specimens::{
    AccessionDetail, AssessExcursionInput, ConfigureLabelPrinterInput,
    ConfigureStorageLocationInput, ContainerHoldView, ContainerLookup, CreateAccessionInput,
    CreateAliquotInput, DisposeContainerInput, ExcursionView, LabelJobView, LabelPrinterView,
    MarkContainerDepletedInput, MoveContainerInput, OrderAccessionContext, PlaceContainerHoldInput,
    PrintLabelsInput, RecordExcursionInput, RecordReceiptDecisionInput, ReleaseContainerHoldInput,
    StorageLocationView,
};
// specimens: end
// analytical: begin
pub use commands::analytical::prep::GetPrepBatchInput;
pub use commands::analytical::qc::GetRunQcInput;
pub use commands::analytical::{
    GetRunInput, ListLotsInput, ListRunsInput, ListSourceRegistrationsInput, ListWorklistInput,
};
pub use lis_analytical::{
    AddPatientPositionInput, AddQcPositionInput, ApproveSourceInput, AssessQcInput,
    CloseDeviationInput, CloseRunInput, CreatePrepBatchInput, CreateRunInput, DeviationTarget,
    DeviationView, FailExecutionInput, InstrumentReadinessView, LotView, OpenDeviationInput,
    PrepBatchView, QcAssessmentView, RecordPrepStepInput, RegisterLotInput, RegisterSourceInput,
    RunDetail, RunQcView, RunSummary, SetInstrumentStatusInput, SourceRegistrationView,
    WithdrawLotInput, WorklistEntry,
};
pub use lis_analytical::{
    CompletePrepBatchInput, PrepBatchState, QcConflictId, QcConflictResolution, QcConflictState,
    QcConflictView, ResolveQcConflictInput,
};
pub use lis_analytical::{ListPrepBatchesInput, PrepBatchSummary, RecordRunPerformingSiteInput};
// analytical: end
// results: begin
mod result_import;
pub use commands::results::{
    GetImportAttemptInput, GetSpecimenResultsInput, ListImportAttemptsInput,
    ListReflexTriggersInput, ListReviewQueueInput, ListValidityPoliciesInput,
};
pub use lis_orders::TriggerAssessmentView;
pub use lis_results::{
    AddOnComponentView, ImportAttemptDetail, ImportAttemptId, ImportAttemptSummary, ImportFileView,
    MeasuredNotOrderedView, MeasurementForm, MeasurementId, QuarantineItemId, QuarantineResolution,
    RecordReflexDecisionInput, RecordResultDispositionInput, RecordValidityAssessmentInput,
    ReflexDecision, ReflexTriggerView, ResolveQuarantineItemInput, ResultId, ResultOutcome,
    ResultRevisionId, ResultRevisionView, RetryImportInput, ReviewQueueEntry, ReviewQueueFilter,
    ReviewQueuePage, ReviewView, SelectResultEvidenceInput, SetAsideKind, SetAsideRowView,
    SpecimenResults, SubmitTechnicalReviewInput, UseMeasuredResultInput, ValidityAssessmentView,
};
// results: end
// reporting: begin
mod report_reads;
mod report_rendering;
pub use commands::report_layouts::PreviewReportLayoutInput;
pub use commands::reporting::{
    GetReportCandidateInput, GetReportInput, GetReportRevisionInput, GetReportWorkspaceInput,
};
pub use lis_reporting::{
    AddDeliveryObligationInput, ApproveReportLayoutInput, CandidateView,
    ClosePreservationIncidentInput, CreateDeliveryDestinationInput, DeliveryQueueFilter,
    DeliveryQueuePage, DestinationState, DestinationView, DraftReportLayoutInput,
    ExportReportPdfInput, ImpactView, ListDeliveryDestinationsInput, ListDeliveryObligationsInput,
    ListReportsInput, ObligationView, PrepareReportCandidateInput, PreservationIncidentView,
    RecordDeliveryAttemptInput, RecordImpactAssessmentInput, RedirectDeliveryObligationInput,
    RejectReportLayoutInput, ReleaseReportInput, ReplaceDeliveryDestinationInput, ReportDetail,
    ReportLayoutsView, ReportListFilter, ReportSummary, ReportWorkspace,
    RetireDeliveryDestinationInput, RevisionDetail, RevisionSummary,
    VerifyDeliveryDestinationInput, WithdrawReportInput,
};
pub use report_rendering::ExportedReportFile;
// reporting: end
// billing: begin
pub use commands::billing::{
    BillingHandoffInput, ExportedHandoffFile, GetClassMappingVersionInput,
    GetServiceDerivationInput,
};
pub use lis_billing::{
    ApproveClassMappingVersion, ApproveServiceDerivation, BillingClassMappingEntry,
    BillingClassMappingVersionId, BillingClassMappingVersionSummary,
    BillingClassMappingVersionView, BillingComponentDisposition, BillingDerivationId,
    BillingDerivationState, BillingFlag, BillingHandoffId, BillingHandoffItemId, BillingWorkFilter,
    BillingWorkItem, BillingWorkState, ConfirmClassMappingVersion, CreateBillingHandoff,
    DerivationClassView, DerivationCompleteness, DerivationComponentView, DerivationSpecimenView,
    DerivationView, DeriveServices, DraftClassMappingVersion, HandoffItemKind, HandoffItemView,
    HandoffOutcome, HandoffStatus, HandoffView, ListBillingWork, RecordHandoffOutcome,
    RetireClassMappingVersion, UpdateClassMappingDraft,
};
// billing: end
// work_queues: begin
pub use commands::work_queues::{
    ListWorkQueueItemsInput, WorkQueueCursor, WorkQueueItem, WorkQueuePage, WorkQueueSummary,
};
// work_queues: end
// backup: begin
pub use commands::backup::{
    AddBackupTargetInput, BackupRunView, BackupSetView, BackupStatus, BeginRestoreInput,
    CloseRecoveryInput, ConfirmRecoveryKitStorageInput, ConfirmRestoreInput, ConfirmRestoreView,
    ExitExportView, ExportRecoveryKitInput, ExportedKit, LimitInput, ListBackupRunsInput,
    ListRestoreTestsInput, RecoveryKitStatus, RecoveryReconciliation, RequestBackupInput,
    RequestExitExportInput, RequestRestoreTestInput, RestoreAvailability, RestoreSessionInput,
    RestoreSessionView, RestoreSourceView, RestoreTestView, RetireBackupTargetInput,
    StageRestoreInput, StagedRestoreView, TargetInput, TargetView,
};
pub use lis_ops::anchors::AnchorSummary;
pub use lis_ops::backup::RunKind as BackupRunKind;
pub use lis_ops::recovery::ChecklistItem;
pub use lis_ops::restore::{RestoreId, RestoreSessionId};
pub use lis_ops::targets::{TargetId, TargetKind, TargetPurpose};
pub use lis_specimens::{NumberFloors, RaiseNumberFloorsInput};
// backup: end
// runtime: begin
mod keep_awake;
mod ops_worker;
pub use commands::dialogs::DialogCommand;
pub use commands::operations::{
    AcceptInstalledVersionInput, AcknowledgeListeningGapInput, AttestHealthCheckInput,
    ListAppRunsInput, ListHealthHistoryInput, RecordUpdateApprovalInput,
    RestoreInstallationKeyInput, UpdateRequest,
};
pub use keep_awake::{Guard, Inhibitor, KeepAwakeView};
pub use keys::{InstallationKeyStateView, KeyState, MemoryKeyStore};
pub use lis_auth::{
    DisableUnlockPinInput, SecurityPolicy, SecurityPolicyView, SeparationPolicy,
    SeparationPolicyView, SetSecurityPolicyInput, SetSeparationPolicyInput, SetUnlockPinInput,
    UnlockPinStatus, UnlockWithPinInput,
};
pub use lis_labels::PrinterDestinations;
pub use lis_ops::disposals::{DisposalMethod, MediaDisposalView, RecordMediaDisposalInput};
pub use lis_ops::health::{CheckResult as HealthCheckView, HealthEventView, HealthState};
pub use lis_ops::policy::OperationsSettings;
pub use lis_ops::runs::{AppRunView, Stamp, StopKind};
pub use lis_ops::settings::{OperationsSettingsView, SetOperationsSettingsInput};
pub use lis_ops::updates::{
    InstallFailure, InstalledVersionView, UpdateApprovalId, UpdateApprovalView, VersionStatus,
    WithdrawUpdateApprovalInput, version_precedence,
};
pub use ops_worker::{HostHealthView, HostView, QuitWarnings};
pub use session::ABSOLUTE_SESSION_LIFETIME;
// runtime: end
// anchors: begin
pub use commands::anchors::{ListAnchorExportsInput, VerifyAuditAnchorsInput};
pub use lis_ops::anchors::{
    AnchorExportView, AnchorStatusView, AnchorVerification, ExportReport as AnchorExportReport,
};
// anchors: end
// result_files: begin
mod result_folders;
pub use commands::result_files::{
    ChosenResultFolder, LoadOutcome, LoadResultFileInput, LoadedResultFile, ResultFolderInput,
};
// result_files: end
// file_templates: begin
pub use commands::file_templates::{
    CheckFileLayoutInput, LayoutSampleHandle, LayoutSampleOpened, PreviewLayoutSampleInput,
};
pub use lis_analytical::{FileLayout, RegisterFileSourceInput};
pub use lis_results::delimited::{LayoutCheck, PreviewLine};
// file_templates: end
// demo: begin
mod demo;
pub use commands::demo::DemoStatus;
pub use demo::DemoDataLoaded;
pub use lis_ops::InstallationKind;
// demo: end
pub use engine::{Engine, EngineConfig, EngineError};
pub use keys::KeyProvider;
pub use outbox_worker::{HandlerError, HandlerFuture, TopicHandler};
pub use session::{IDLE_TIMEOUT, SessionInfo};

// Financial intake and collection corrections.
pub use lis_identity::{
    GetPatientBillingProfileInput, PatientBillingProfile, PatientBillingProfileView,
    SetPatientBillingProfileInput,
};
pub use lis_orders::{
    BillingReviewRecord, OrderBillingContext, OrderBillingView, SetOrderBillingContextInput,
    UpdateBillingReviewItemInput,
};
pub use lis_specimens::{
    CollectionHistory, CorrectCollectionTimeInput, ResolveCollectionCorrectionInput,
};

pub use lis_billing::{RecordHandoffFollowupInput, ResolveBillingContextConflictInput};
pub use lis_orders::ListBillingReviewItemsInput;

pub use commands::specimens::material::{
    GetContainerHistoryInput, GetExcursionInput, ListExcursionsInput,
};
pub use lis_specimens::{ExcursionPage, MaterialHistoryPage};

pub use lis_identity::{
    IntakeDeferralView, ListIntakeDeferralsInput, RecordIntakeDeferralInput,
    ResolveIntakeDeferralInput,
};

pub use lis_identity::{
    ClientLocationView, DirectoryHistoryEntry, DirectoryHistoryInput, UpdateClient,
    UpdateClientLocation, UpdatePractitioner,
};

pub use commands::analytical::qc::GetDeviationInput;
pub use commands::results::GetReceiptImportAttemptInput;

pub use commands::reporting::GetDeliveryObligationInput;
