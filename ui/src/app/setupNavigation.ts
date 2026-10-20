import {
  Atom,
  Beaker,
  BookOpen,
  Building2,
  ClipboardCheck,
  DatabaseBackup,
  FlaskConical,
  FolderOutput,
  HardDrive,
  HeartPulse,
  KeyRound,
  Landmark,
  Microscope,
  PackageCheck,
  PackageOpen,
  Settings2,
  SlidersHorizontal,
  Stethoscope,
  TestTube,
  Users,
  Waypoints,
} from "lucide-react";
import { CATALOG_REVIEW_PATH } from "@/lib/catalogQueries";
import type { NavSection } from "./navigation";

/** Who manages or approves configuration; also on the bench setup entry. */
export const CONFIGURATION_ROLES = ["manage_configuration", "approve_configuration"];
const SYSTEM_ROLES = ["backup_restore", "export", "manage_configuration"];

/** The setup and system areas, after the work areas: collapsed until someone opens them. */
export const setupSections: readonly NavSection[] = [
  {
    heading: "Test catalog",
    icon: BookOpen,
    collapsedByDefault: true,
    screens: [
      {
        path: "/configuration/orderables",
        label: "Tests and panels",
        icon: FlaskConical,
        page: () =>
          import("@/features/configuration/orderables/OrderablesPage").then(
            (m) => m.OrderablesPage,
          ),
        capabilities: CONFIGURATION_ROLES,
      },
      {
        path: "/configuration/methods",
        label: "Methods",
        icon: Beaker,
        page: () =>
          import("@/features/configuration/methods/MethodsPage").then((m) => m.MethodsPage),
        capabilities: CONFIGURATION_ROLES,
      },
      {
        path: "/configuration/analytes",
        label: "Analytes",
        icon: Atom,
        page: () =>
          import("@/features/configuration/analytes/AnalytesPage").then((m) => m.AnalytesPage),
        capabilities: CONFIGURATION_ROLES,
      },
      {
        path: "/configuration/specimen-types",
        label: "Specimen types",
        icon: TestTube,
        page: () =>
          import("@/features/configuration/validity/SpecimenTypesPage").then(
            (m) => m.SpecimenTypesPage,
          ),
        capabilities: CONFIGURATION_ROLES,
      },
    ],
  },
  {
    heading: "Laboratory setup",
    icon: Building2,
    collapsedByDefault: true,
    screens: [
      {
        path: "/setup/material",
        label: "Material configuration",
        icon: PackageCheck,
        page: () =>
          import("@/features/accessioning/material-config/MaterialConfigurationPage").then(
            (m) => m.MaterialConfigurationPage,
          ),
        capabilities: ["manage_configuration"],
      },
      {
        path: "/configuration/laboratory",
        label: "Laboratory profile",
        icon: Landmark,
        page: () =>
          import("@/features/configuration/laboratory/LaboratoryPage").then(
            (m) => m.LaboratoryPage,
          ),
        capabilities: ["manage_configuration"],
      },
      {
        path: CATALOG_REVIEW_PATH,
        label: "Catalog review",
        icon: ClipboardCheck,
        page: () =>
          import("@/features/configuration/catalog-review/CatalogReviewPage").then(
            (m) => m.CatalogReviewPage,
          ),
        capabilities: CONFIGURATION_ROLES,
      },
      {
        path: "/configuration/clients",
        label: "Clients",
        icon: Building2,
        page: () =>
          import("@/features/configuration/clients/ClientsPage").then((m) => m.ClientsPage),
        capabilities: ["manage_configuration"],
      },
      {
        path: "/configuration/practitioners",
        label: "Practitioners",
        icon: Stethoscope,
        page: () =>
          import("@/features/configuration/practitioners/PractitionersPage").then(
            (m) => m.PractitionersPage,
          ),
        capabilities: ["manage_configuration"],
      },
      {
        path: "/configuration/destinations",
        label: "Delivery destinations",
        icon: Waypoints,
        page: () =>
          import("@/features/configuration/destinations/DestinationsPage").then(
            (m) => m.DestinationsPage,
          ),
        capabilities: ["manage_configuration"],
      },
      {
        path: "/configuration/instruments",
        label: "Instruments",
        icon: Microscope,
        page: () =>
          import("@/features/configuration/instruments/InstrumentsPage").then(
            (m) => m.InstrumentsPage,
          ),
        capabilities: ["manage_configuration"],
      },
      {
        path: "/users",
        label: "Users",
        icon: Users,
        page: () => import("@/features/users/UsersPage").then((m) => m.UsersPage),
        capabilities: ["manage_users", "manage_quality"],
      },
      {
        path: "/configuration/starter-data",
        label: "Starter data",
        icon: PackageOpen,
        page: () => import("@/features/starter/StarterDataPage").then((m) => m.StarterDataPage),
        capabilities: CONFIGURATION_ROLES,
      },
    ],
  },
  {
    heading: "System",
    icon: Settings2,
    collapsedByDefault: true,
    screens: [
      {
        path: "/operations/health",
        label: "Host health",
        icon: HeartPulse,
        page: () => import("@/features/operations/health/HealthPage").then((m) => m.HealthPage),
        // Everyone reaches it from the top-bar health badge; the entry is for operators.
        capabilities: SYSTEM_ROLES,
      },
      {
        path: "/operations/backups",
        label: "Backups",
        icon: DatabaseBackup,
        page: () => import("@/features/operations/backups/BackupsPage").then((m) => m.BackupsPage),
        capabilities: ["backup_restore"],
        tabbed: true,
      },
      {
        path: "/operations/recovery-kit",
        label: "Recovery kit",
        icon: KeyRound,
        page: () =>
          import("@/features/operations/kit/RecoveryKitPage").then((m) => m.RecoveryKitPage),
        capabilities: ["backup_restore"],
      },
      {
        path: "/operations/versions",
        label: "Versions and updates",
        icon: PackageCheck,
        page: () =>
          import("@/features/operations/updates/VersionsPage").then((m) => m.VersionsPage),
        capabilities: CONFIGURATION_ROLES,
        tabbed: true,
      },
      {
        path: "/operations/settings",
        label: "Operations settings",
        icon: SlidersHorizontal,
        page: () =>
          import("@/features/operations/settings/OperationsSettingsPage").then(
            (m) => m.OperationsSettingsPage,
          ),
        capabilities: [
          "manage_configuration",
          "approve_configuration",
          "manage_users",
          "manage_quality",
        ],
      },
      {
        path: "/operations/media-disposals",
        label: "Media disposals",
        icon: HardDrive,
        page: () =>
          import("@/features/operations/disposals/MediaDisposalsPage").then(
            (m) => m.MediaDisposalsPage,
          ),
        capabilities: ["backup_restore"],
      },
      {
        path: "/operations/exit-export",
        label: "Exit export",
        icon: FolderOutput,
        page: () =>
          import("@/features/operations/export/ExitExportPage").then((m) => m.ExitExportPage),
        capabilities: ["export"],
      },
    ],
  },
];
