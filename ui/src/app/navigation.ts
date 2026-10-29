import {
  Anchor,
  ClipboardCheck,
  ClipboardList,
  ContactRound,
  FileCheck,
  FileInput,
  FileOutput,
  Files,
  FileText,
  Inbox,
  LayoutTemplate,
  ListChecks,
  type LucideIcon,
  Microscope,
  Receipt,
  ReceiptText,
  ScanBarcode,
  ScrollText,
  Send,
  ShieldCheck,
  Tags,
  TestTubes,
  UserSearch,
  Wrench,
} from "lucide-react";
import type { ComponentType } from "react";
import { CONFIGURATION_ROLES, setupSections } from "./setupNavigation";

/** Loads a screen's module on first use, so the shell and sign-in start without it (NFR-33). */
export type PageLoader = () => Promise<ComponentType>;

/** Shown beside an entry: every queue's count (`ALL_QUEUES`) or one queue's, by engine code. */
export const ALL_QUEUES = "all";

export interface Screen {
  path: string;
  /** Equal to the screen's page title, so the navigation and the `h1` say the same thing. */
  label: string;
  icon: LucideIcon;
  page: PageLoader;
  /**
   * The capabilities the screen's commands need; the navigation shows it to people holding
   * any of them and hides it from others. Omitted: every session.
   */
  capabilities?: readonly string[];
  /** The `list_work_queues` queue whose count shows beside the entry. */
  queue?: string;
  /** Its tabs are path segments below `path`; its route takes an optional tab. */
  tabbed?: boolean;
}

export interface NavSection {
  /** The work area's heading; the first section has none. */
  heading?: string;
  /** Stable work-area symbol, also used by the compact navigation rail. */
  icon?: LucideIcon;
  screens: readonly Screen[];
}

/**
 * The signed-in screens by work area, in the laboratory's flow (order, accession, bench,
 * review, release, deliver, bill), then oversight and setup: the one table behind the side
 * navigation and the routes. A section shows when one of its screens does.
 */
export const navigation: readonly NavSection[] = [
  {
    screens: [
      {
        path: "/work-queues",
        label: "Work queues",
        icon: Inbox,
        page: () => import("@/features/workqueues/WorkQueuesPage").then((m) => m.WorkQueuesPage),
        queue: ALL_QUEUES,
      },
    ],
  },
  {
    heading: "Front desk",
    icon: ContactRound,
    screens: [
      {
        path: "/patients",
        label: "Patients",
        icon: UserSearch,
        page: () =>
          import("@/features/accessioning/patients/PatientsPage").then((m) => m.PatientsPage),
        capabilities: ["enter_orders", "manage_quality"],
      },
      {
        path: "/orders",
        label: "Orders",
        icon: ClipboardList,
        page: () => import("@/features/accessioning/orders/OrdersPage").then((m) => m.OrdersPage),
        capabilities: ["enter_orders"],
        queue: "orders_needing_decision",
      },
      {
        path: "/patients/incomplete-intake",
        label: "Incomplete identity intake",
        icon: UserSearch,
        page: () =>
          import("@/features/accessioning/patients/IncompleteIntakePage").then(
            (m) => m.IncompleteIntakePage,
          ),
        capabilities: ["enter_orders"],
      },
      {
        path: "/specimens/storage",
        label: "Storage excursions",
        icon: ScanBarcode,
        page: () =>
          import("@/features/accessioning/specimens/StorageExcursionsPage").then(
            (m) => m.StorageExcursionsPage,
          ),
        capabilities: ["accession", "manage_quality"],
      },
      {
        path: "/containers",
        label: "Container lookup",
        icon: ScanBarcode,
        page: () =>
          import("@/features/accessioning/specimens/ContainerLookupPage").then(
            (m) => m.ContainerLookupPage,
          ),
        capabilities: ["accession", "manage_quality"],
      },
    ],
  },
  {
    heading: "Bench",
    icon: Microscope,
    screens: [
      {
        path: "/bench/worklist",
        label: "Worklist",
        icon: ListChecks,
        page: () => import("@/features/analytics/runs/WorklistPage").then((m) => m.WorklistPage),
        capabilities: ["perform_analysis"],
      },
      {
        path: "/bench/runs",
        label: "Runs",
        icon: TestTubes,
        page: () => import("@/features/analytics/runs/RunsPage").then((m) => m.RunsPage),
        capabilities: ["perform_analysis"],
      },
      {
        path: "/bench/setup",
        label: "Bench setup",
        icon: Wrench,
        page: () =>
          import("@/features/analytics/bench/BenchSetupPage").then((m) => m.BenchSetupPage),
        capabilities: ["perform_analysis", "manage_quality", ...CONFIGURATION_ROLES],
        tabbed: true,
      },
    ],
  },
  {
    heading: "Review",
    icon: ClipboardCheck,
    screens: [
      {
        path: "/review/queue",
        label: "Result review",
        icon: ClipboardCheck,
        page: () =>
          import("@/features/analytics/review/ReviewQueuePage").then((m) => m.ReviewQueuePage),
        capabilities: ["review_results"],
        queue: "review_ready",
      },
      {
        path: "/review/imports",
        label: "Result imports",
        icon: FileInput,
        page: () =>
          import("@/features/analytics/imports/ImportAttemptsPage").then(
            (m) => m.ImportAttemptsPage,
          ),
        capabilities: ["review_results"],
        queue: "import_quarantine",
      },
    ],
  },
  {
    heading: "Reports",
    icon: Files,
    screens: [
      {
        path: "/reports/release",
        label: "Release queue",
        icon: FileCheck,
        page: () =>
          import("@/features/reports/release/ReleaseQueuePage").then((m) => m.ReleaseQueuePage),
        capabilities: ["release_reports"],
        queue: "release_ready",
      },
      {
        path: "/delivery",
        label: "Delivery",
        icon: Send,
        page: () =>
          import("@/features/reports/delivery/DeliveryQueuePage").then((m) => m.DeliveryQueuePage),
        capabilities: ["deliver_reports"],
        queue: "delivery_work",
      },
      {
        path: "/reports",
        label: "Reports",
        icon: FileText,
        page: () => import("@/features/reports/history/ReportsPage").then((m) => m.ReportsPage),
        capabilities: ["deliver_reports", "manage_quality"],
      },
      {
        path: "/reports/templates",
        label: "Report templates",
        icon: LayoutTemplate,
        page: () =>
          import("@/features/reports/templates/ReportTemplatesPage").then(
            (m) => m.ReportTemplatesPage,
          ),
        capabilities: ["manage_configuration", "manage_quality"],
      },
    ],
  },
  {
    heading: "Billing",
    icon: ReceiptText,
    screens: [
      {
        path: "/billing/review",
        label: "Billing follow-up",
        icon: ClipboardList,
        page: () =>
          import("@/features/accessioning/billing/BillingReviewPage").then(
            (m) => m.BillingReviewPage,
          ),
        capabilities: ["billing", "enter_orders"],
      },
      {
        path: "/billing/work",
        label: "Billing work",
        icon: Receipt,
        page: () =>
          import("@/features/billing/work/BillingWorkPage").then((m) => m.BillingWorkPage),
        capabilities: ["billing"],
        queue: "billing_work",
      },
      {
        path: "/billing/handoffs",
        label: "Billing handoffs",
        icon: FileOutput,
        page: () => import("@/features/billing/handoffs/HandoffsPage").then((m) => m.HandoffsPage),
        capabilities: ["billing"],
      },
      {
        path: "/billing/class-mappings",
        label: "Billing class mappings",
        icon: Tags,
        page: () =>
          import("@/features/billing/mappings/ClassMappingsPage").then((m) => m.ClassMappingsPage),
        capabilities: ["billing", "approve_configuration"],
      },
    ],
  },
  {
    heading: "Quality and audit",
    icon: ShieldCheck,
    screens: [
      {
        path: "/audit",
        label: "Audit trail",
        icon: ScrollText,
        page: () => import("@/features/audit/AuditTrailPage").then((m) => m.AuditTrailPage),
        capabilities: ["view_audit"],
      },
      {
        path: "/audit/anchors",
        label: "Audit anchors",
        icon: Anchor,
        page: () => import("@/features/audit/anchors/AnchorsPage").then((m) => m.AnchorsPage),
        capabilities: ["view_audit"],
      },
    ],
  },
  ...setupSections,
];

/** The landing page after sign-in, for every role. */
export const HOME_PATH = "/work-queues";

/** Where the top-bar scan field sends a barcode: the container lookup, by label code. */
export function containerLookupPath(labelCode: string) {
  return `/containers?${new URLSearchParams({ labelCode })}`;
}

/** The component gallery, in development builds only. */
export const GALLERY_PATH = "/dev/components";
