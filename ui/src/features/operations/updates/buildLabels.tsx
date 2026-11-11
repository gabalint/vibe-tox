import { StatusBadge } from "@/components/StatusBadge";
import { Timestamp } from "@/components/Timestamp";
import type { AvailableBuild } from "@/generated/AvailableBuild";

/** Whether a published build can replace the running version; only newer versions install. */
export function BuildStatus({ build, running }: { build: AvailableBuild; running: string }) {
  if (build.version === running) return <StatusBadge tone="neutral">Running</StatusBadge>;
  return build.newer ? (
    <StatusBadge tone="info">Newer</StatusBadge>
  ) : (
    <StatusBadge tone="neutral">Older</StatusBadge>
  );
}

/** The first 12 characters of a commit SHA, as GitHub shows it. */
export const shortCommit = (commit: string | undefined) => commit?.slice(0, 12) ?? "Not given";

export const PublishedAt = ({ build }: { build: AvailableBuild }) =>
  build.publishedAt ? <Timestamp value={build.publishedAt} /> : "Not given";
