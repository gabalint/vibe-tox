import type { UseQueryResult } from "@tanstack/react-query";
import { Link, useLocation } from "react-router";
import { QueryContent } from "@/components/QueryContent";
import { Section } from "@/components/Section";
import type { CatalogVersionSummary } from "@/generated/CatalogVersionSummary";
import type { CommandError } from "@/generated/CommandError";
import { useFormatTime } from "@/lib/format";
import { usePersonText } from "@/lib/person";
import styles from "./configuration.module.css";
import { effectiveText, VersionStatusBadge } from "./VersionFacts";

interface VersionListProps {
  versions: UseQueryResult<CatalogVersionSummary[], CommandError>;
  /** The version whose content the page shows. */
  shownId: string | undefined;
}

/**
 * Every version of one record, newest first: its status, effective period, and who drafted
 * and approved it. Each is a link to `?version=<number>`, which shows its content; following
 * one replaces the history entry and keeps the list the page was opened from.
 */
export function VersionList({ versions, shownId }: VersionListProps) {
  const { state } = useLocation();
  const person = usePersonText();
  const formatTime = useFormatTime();
  return (
    <div className={styles.versionColumn}>
      <Section title="Versions">
        <QueryContent
          query={versions}
          what="versions"
          loadingRows={3}
          empty={{
            when: (rows) => rows.length === 0,
            title: "No versions yet",
            description: "No version has been drafted yet.",
          }}
        >
          {(rows) => (
            <ul className={styles.versionItems}>
              {newestFirst(rows).map((version) => (
                <li key={version.id}>
                  <Link
                    to={{ search: `?version=${version.version}` }}
                    replace
                    state={state}
                    aria-current={version.id === shownId ? "true" : undefined}
                    className={styles.versionLink}
                  >
                    <span className={styles.versionTitle}>
                      Version {version.version}
                      <VersionStatusBadge version={version} />
                    </span>
                    <span className={styles.muted}>{effectiveText(version, formatTime)}</span>
                    <span className={styles.muted}>{authorsText(version, person)}</span>
                  </Link>
                </li>
              ))}
            </ul>
          )}
        </QueryContent>
      </Section>
    </div>
  );
}

/** "Drafted by you, approved by Test Approver", with the engine's display names. */
function authorsText(
  version: CatalogVersionSummary,
  person: (userId: string, display?: string) => string,
): string {
  const drafted = `Drafted by ${person(version.createdBy, version.createdByDisplay)}`;
  if (version.approvedBy === null) return drafted;
  return `${drafted}, approved by ${person(version.approvedBy, version.approvedByDisplay)}`;
}

function newestFirst(versions: readonly CatalogVersionSummary[]): CatalogVersionSummary[] {
  return [...versions].sort((a, b) => b.version - a.version);
}
