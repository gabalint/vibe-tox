import type { PageLoader } from "./navigation";

/**
 * Screens reached from other screens rather than from the navigation, each loaded on first use.
 * A tabbed screen's tabs are path segments (`/operations/backups/runs`) of one route with an
 * optional `:tab`, so moving between tabs keeps the screen mounted; its base path shows the
 * first tab.
 */
export const detailScreens: readonly { path: string; page: PageLoader; tabbed?: boolean }[] = [
  {
    path: "/quality/deviations/:deviationId",
    page: () => import("@/features/analytics/qc/DeviationPage").then((m) => m.DeviationPage),
  },
  {
    path: "/patients/register",
    page: () =>
      import("@/features/accessioning/patients/RegisterPatientPage").then(
        (m) => m.RegisterPatientPage,
      ),
  },
  {
    path: "/patients/:subjectId",
    page: () =>
      import("@/features/accessioning/patients/PatientDetailPage").then((m) => m.PatientDetailPage),
  },
  {
    path: "/orders/new",
    page: () => import("@/features/accessioning/orders/NewOrderPage").then((m) => m.NewOrderPage),
  },
  {
    path: "/orders/:orderId",
    page: () =>
      import("@/features/accessioning/orders/OrderDetailPage").then((m) => m.OrderDetailPage),
  },
  {
    path: "/orders/:orderId/accession",
    page: () =>
      import("@/features/accessioning/specimens/NewAccessionPage").then((m) => m.NewAccessionPage),
  },
  {
    path: "/accessions/:accessionId",
    page: () =>
      import("@/features/accessioning/specimens/AccessionPage").then((m) => m.AccessionPage),
  },
  {
    path: "/accessions/:accessionId/collection-review",
    page: () =>
      import("@/features/accessioning/billing/CollectionReviewPage").then(
        (m) => m.CollectionReviewPage,
      ),
  },
  {
    path: "/configuration/clients/:clientId",
    page: () => import("@/features/configuration/clients/ClientPage").then((m) => m.ClientPage),
  },
  {
    path: "/configuration/methods/:methodId",
    page: () => import("@/features/configuration/methods/MethodPage").then((m) => m.MethodPage),
  },
  {
    path: "/configuration/orderables/:orderableId",
    page: () =>
      import("@/features/configuration/orderables/OrderablePage").then((m) => m.OrderablePage),
  },
  {
    path: "/configuration/specimen-types/:specimenTypeId",
    page: () =>
      import("@/features/configuration/validity/ValidityPolicyPage").then(
        (m) => m.ValidityPolicyPage,
      ),
  },
  {
    path: "/bench/runs/:runId",
    page: () => import("@/features/analytics/runs/RunPage").then((m) => m.RunPage),
  },
  {
    path: "/bench/runs/:runId/qc",
    page: () => import("@/features/analytics/qc/RunQcPage").then((m) => m.RunQcPage),
    tabbed: true,
  },
  {
    path: "/bench/setup/instruments/:instrumentId/file-template",
    page: () =>
      import("@/features/analytics/fileTemplates/FileTemplatePage").then((m) => m.FileTemplatePage),
  },
  {
    path: "/bench/setup/instruments/:instrumentId/sources/:registrationId",
    page: () =>
      import("@/features/analytics/fileTemplates/FileSourcePage").then((m) => m.FileSourcePage),
  },
  {
    path: "/review/specimens/:specimenId",
    page: () =>
      import("@/features/analytics/review/SpecimenResultsPage").then((m) => m.SpecimenResultsPage),
  },
  {
    path: "/review/imports/:importAttemptId",
    page: () =>
      import("@/features/analytics/imports/ImportAttemptPage").then((m) => m.ImportAttemptPage),
    // A result file's attempt has tabs: quarantined rows and rows set aside.
    tabbed: true,
  },
  {
    path: "/work-queues/:queue",
    page: () => import("@/features/workqueues/WorkQueuePage").then((m) => m.WorkQueuePage),
  },
  {
    path: "/reports/specimens/:specimenId",
    page: () =>
      import("@/features/reports/release/ReportWorkspacePage").then((m) => m.ReportWorkspacePage),
  },
  {
    path: "/reports/specimens/:specimenId/candidates/:candidateId",
    page: () => import("@/features/reports/release/CandidatePage").then((m) => m.CandidatePage),
  },
  {
    path: "/reports/revisions/:revisionId",
    page: () => import("@/features/reports/history/RevisionPage").then((m) => m.RevisionPage),
  },
  {
    path: "/reports/:reportId",
    page: () => import("@/features/reports/history/ReportPage").then((m) => m.ReportPage),
  },
  {
    path: "/billing/class-mappings/:versionId",
    page: () =>
      import("@/features/billing/mappings/ClassMappingPage").then((m) => m.ClassMappingPage),
  },
  {
    path: "/billing/orders/:orderId",
    page: () =>
      import("@/features/accessioning/billing/BillingOrderPage").then((m) => m.BillingOrderPage),
  },
  {
    path: "/billing/derivations/:derivationId",
    page: () =>
      import("@/features/billing/derivations/DerivationPage").then((m) => m.DerivationPage),
  },
  {
    path: "/billing/handoffs/:handoffId",
    page: () => import("@/features/billing/handoffs/HandoffPage").then((m) => m.HandoffPage),
  },
  {
    path: "/operations/health/history",
    page: () =>
      import("@/features/operations/health/HealthHistoryPage").then((m) => m.HealthHistoryPage),
    tabbed: true,
  },
  {
    path: "/operations/recovery",
    page: () =>
      import("@/features/operations/reconciliation/ReconciliationPage").then(
        (m) => m.ReconciliationPage,
      ),
  },
  {
    path: "/about",
    page: () => import("@/features/system/AboutPage").then((m) => m.AboutPage),
  },
  {
    path: "/configuration/starter-data/load",
    page: () => import("@/features/starter/LoadCatalogPage").then((m) => m.LoadCatalogPage),
  },
];
