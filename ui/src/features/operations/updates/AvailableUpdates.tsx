import { useMemo } from "react";
import { Alert } from "@/components/Alert";
import { Button } from "@/components/Button";
import { DataGrid } from "@/components/DataGrid";
import { DetailLayout, DetailPanel } from "@/components/DetailPanel";
import { ErrorState } from "@/components/ErrorState";
import { Facts } from "@/components/Facts";
import { FindCount } from "@/components/FindField";
import { QueryContent } from "@/components/QueryContent";
import { Section } from "@/components/Section";
import { Toolbar } from "@/components/Toolbar";
import type { AvailableBuild } from "@/generated/AvailableBuild";
import type { UpdateFeedView } from "@/generated/UpdateFeedView";
import { useCommandQuery, useHasCapability } from "@/lib/commands";
import { useOpenRecord } from "@/lib/openRecord";
import { openRecordPanel } from "@/lib/openRecordPanel";
import layout from "@/styles/layout.module.css";
import { CHECK_RELEASE_UPDATE, LIST_NIGHTLY_BUILDS, MANAGE_CONFIGURATION } from "../contract";
import { BuildStatus, buildColumns, PublishedAt, shortCommit } from "./buildColumns";
import { UpdateActions } from "./UpdateActions";

const buildId = (row: AvailableBuild) => row.version;

/**
 * Updates published for this app (ADR 0020): the newest official release, and, only while a
 * nightly build runs, the retained nightly builds for internal testing. The app asks the release
 * repository when this tab opens or on Check again, never on a schedule (NFR-24). Someone with
 * `manage_configuration` downloads, approves and installs a newer build here.
 */
export function AvailableUpdates() {
  const release = useCommandQuery<UpdateFeedView>(CHECK_RELEASE_UPDATE);
  const nightly = useCommandQuery<UpdateFeedView>(LIST_NIGHTLY_BUILDS);
  return (
    <>
      <QueryContent query={release} what="official release">
        {(feed) =>
          feed.configured ? (
            <ReleaseSection feed={feed} onCheck={() => void release.refetch()} />
          ) : (
            <Alert tone="info" title="Updates are not available in this build">
              This build cannot download updates. Install a release from the project’s release page
              once; later releases can then be installed here.
            </Alert>
          )
        }
      </QueryContent>
      {nightly.data?.offered && nightly.data.configured && (
        <NightlySection feed={nightly.data} onCheck={() => void nightly.refetch()} />
      )}
      {nightly.isError && (
        <ErrorState
          title="The nightly builds could not be checked"
          error={nightly.error}
          onRetry={() => void nightly.refetch()}
        />
      )}
    </>
  );
}

function ReleaseSection({ feed, onCheck }: { feed: UpdateFeedView; onCheck: () => void }) {
  const canInstall = useHasCapability(MANAGE_CONFIGURATION);
  const latest = feed.builds[0];
  return (
    <Section title="Official release" actions={<Button onPress={onCheck}>Check again</Button>}>
      {!latest && <p>No official release with in-app updates is published yet.</p>}
      {latest && !latest.newer && (
        <p>
          Version {feed.runningVersion} is running. The newest official release is {latest.version}.
        </p>
      )}
      {latest?.newer && (
        <div className={layout.stack}>
          <p>
            Version {latest.version} is available; version {feed.runningVersion} is running.
          </p>
          <BuildFacts build={latest} />
          {canInstall && <UpdateActions build={latest} />}
        </div>
      )}
    </Section>
  );
}

function NightlySection({ feed, onCheck }: { feed: UpdateFeedView; onCheck: () => void }) {
  const record = useOpenRecord();
  const columns = useMemo(() => buildColumns(feed.runningVersion), [feed.runningVersion]);
  const panel = openRecordPanel(
    record,
    feed.builds,
    buildId,
    (build) => (
      <BuildPanel
        build={build}
        running={feed.runningVersion}
        onClose={record.close}
        onPrevious={record.previous}
        onNext={record.next}
      />
    ),
    <p>This nightly build is no longer published.</p>,
  );
  return (
    <Section
      title="Nightly builds for internal testing"
      actions={<Button onPress={onCheck}>Check again</Button>}
    >
      <p>
        Builds of the development line, newest first. They are for testing with synthetic data only
        and are offered because a nightly build is running.
      </p>
      <DetailLayout panel={panel}>
        <Toolbar
          variant="collection"
          count={
            <FindCount
              shown={feed.builds.length}
              total={feed.builds.length}
              noun="nightly builds"
              one="nightly build"
            />
          }
        />
        {feed.builds.length === 0 ? (
          <p>No nightly build is published yet.</p>
        ) : (
          <DataGrid
            label="Nightly builds"
            columns={columns}
            rows={feed.builds}
            getRowId={buildId}
            openRowId={record.openId}
            onOrderChange={record.followOrder}
            onRowAction={(row, order) => record.open(row.version, order)}
          />
        )}
      </DetailLayout>
    </Section>
  );
}

interface BuildPanelProps {
  build: AvailableBuild;
  running: string;
  onClose: () => void;
  onPrevious: (() => void) | undefined;
  onNext: (() => void) | undefined;
}

/** One nightly build with its changes; a newer build can be installed from here. */
function BuildPanel({ build, running, ...navigation }: BuildPanelProps) {
  const canInstall = useHasCapability(MANAGE_CONFIGURATION);
  return (
    <DetailPanel
      identifier={`Version ${build.version}`}
      status={<BuildStatus build={build} running={running} />}
      {...navigation}
    >
      <BuildFacts build={build} />
      {canInstall && build.newer && <UpdateActions build={build} />}
      {!build.newer && build.version !== running && (
        <p>Only a version newer than the running one can be installed.</p>
      )}
    </DetailPanel>
  );
}

function BuildFacts({ build }: { build: AvailableBuild }) {
  return (
    <>
      <Facts
        items={[
          { label: "Built", value: <PublishedAt build={build} /> },
          ...(build.channel === "nightly"
            ? [{ label: "Commit", value: shortCommit(build.commit) }]
            : []),
        ]}
      />
      <h3 className={layout.subheading}>Changes</h3>
      {build.changes.length === 0 ? (
        <p>No change summary was published.</p>
      ) : (
        <ul aria-label="Changes" className={layout.list}>
          {build.changes.map((change, index) => (
            // Published lines may repeat; their position identifies them.
            // biome-ignore lint/suspicious/noArrayIndexKey: a static, ordered list
            <li key={index}>{change}</li>
          ))}
        </ul>
      )}
    </>
  );
}
