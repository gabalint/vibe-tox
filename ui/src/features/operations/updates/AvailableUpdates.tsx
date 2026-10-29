import { useState } from "react";
import { ToggleButton, ToggleButtonGroup } from "react-aria-components";
import { Alert } from "@/components/Alert";
import { Button } from "@/components/Button";
import { ErrorState } from "@/components/ErrorState";
import { LoadingState } from "@/components/LoadingState";
import { Section } from "@/components/Section";
import type { AvailableBuild } from "@/generated/AvailableBuild";
import type { UpdateChannel } from "@/generated/UpdateChannel";
import type { UpdateFeedView } from "@/generated/UpdateFeedView";
import { useCommandQuery, useHasCapability } from "@/lib/commands";
import layout from "@/styles/layout.module.css";
import { CHECK_RELEASE_UPDATE, LIST_NIGHTLY_BUILDS, MANAGE_CONFIGURATION } from "../contract";
import styles from "./AvailableUpdates.module.css";
import { BuildStatus, PublishedAt, shortCommit } from "./buildLabels";
import { UpdateActions } from "./UpdateActions";
import { UpdateStatus } from "./UpdateStatus";

/**
 * Updates published for this app (ADR 0020): first whether the app is up to date, with Refresh;
 * then a Nightly / Release switch while a nightly build runs, and the newest build of the chosen
 * channel with its changes and one Update action; older builds fold away. The app asks the
 * release repository when this tab opens or on Refresh, never on a schedule (NFR-24).
 */
export function AvailableUpdates() {
  const release = useCommandQuery<UpdateFeedView>(CHECK_RELEASE_UPDATE);
  const nightly = useCommandQuery<UpdateFeedView>(LIST_NIGHTLY_BUILDS);
  const [chosen, setChosen] = useState<UpdateChannel | null>(null);
  if (release.isPending || nightly.isPending) {
    return <LoadingState label="Checking for updates…" />;
  }
  const feed = release.data ?? nightly.data;
  if (feed && !feed.configured) {
    return (
      <Alert tone="info" title="Updates are not available in this build">
        This build cannot download updates. Install a release from the project’s release page once;
        later releases can then be installed here.
      </Alert>
    );
  }
  const nightlyOffered = Boolean(nightly.data?.offered);
  const channel: UpdateChannel = chosen ?? (nightlyOffered ? "nightly" : "release");
  const shown = channel === "nightly" ? nightly : release;
  // One refresh for both feeds: it shows when they were read and spins while either is read.
  const refresh = {
    dataUpdatedAt: Math.min(release.dataUpdatedAt, nightly.dataUpdatedAt),
    isFetching: release.isFetching || nightly.isFetching,
    refetch: () => Promise.all([release.refetch(), nightly.refetch()]),
  };
  const feeds = [nightly.data, release.data].filter((read): read is UpdateFeedView => !!read);
  return (
    <div className={layout.stack}>
      <UpdateStatus
        running={feed?.runningVersion}
        feeds={feeds}
        failed={release.isError || nightly.isError}
        refresh={refresh}
      />
      <Section title="Available versions">
        {nightlyOffered && (
          <ToggleButtonGroup
            aria-label="Update channel"
            className={styles.switch}
            selectionMode="single"
            disallowEmptySelection
            selectedKeys={[channel]}
            onSelectionChange={(keys) => setChosen([...keys][0] as UpdateChannel)}
          >
            <ToggleButton id="nightly">Nightly</ToggleButton>
            <ToggleButton id="release">Release</ToggleButton>
          </ToggleButtonGroup>
        )}
        {shown.isError ? (
          <ErrorState
            title={`The ${channel} updates could not be checked`}
            error={shown.error}
            onRetry={() => void shown.refetch()}
          />
        ) : (
          shown.data && <ChannelUpdates feed={shown.data} channel={channel} />
        )}
      </Section>
    </div>
  );
}

/** The newest version of one channel; it includes every earlier one. */
function ChannelUpdates({ feed, channel }: { feed: UpdateFeedView; channel: UpdateChannel }) {
  const [newest] = feed.builds;
  if (!newest) {
    return (
      <p>
        {channel === "nightly"
          ? "No nightly build is published yet."
          : "No official release with in-app updates is published yet."}
      </p>
    );
  }
  return <Build build={newest} running={feed.runningVersion} />;
}

/**
 * One published version with Update at its bottom right. A nightly build also shows when it was
 * built, its commit and what changed; an official release shows its version and release date.
 */
function Build({ build, running }: { build: AvailableBuild; running: string }) {
  const canInstall = useHasCapability(MANAGE_CONFIGURATION);
  const nightly = build.channel === "nightly";
  return (
    <article aria-label={`Version ${build.version}`} className={styles.build}>
      <div className={layout.stack}>
        <div>
          <p className={styles.version}>
            {build.version} <BuildStatus build={build} running={running} />
          </p>
          <p className={styles.meta}>
            {nightly ? (
              <>
                Nightly build, built <PublishedAt build={build} /> · commit{" "}
                {shortCommit(build.commit)}
              </>
            ) : (
              <PublishedAt build={build} />
            )}
          </p>
        </div>
        {nightly && (
          <>
            <h3 className={layout.subheading}>What’s new</h3>
            <Changes changes={build.changes} />
          </>
        )}
      </div>
      {build.newer && canInstall && <UpdateActions build={build} />}
    </article>
  );
}

const CHANGES_SHOWN = 3;

/** A version's published change lines: the first three, the rest on request. */
function Changes({ changes }: { changes: string[] }) {
  const [all, setAll] = useState(false);
  if (changes.length === 0) {
    return <p>No change summary was published.</p>;
  }
  const hidden = changes.length - CHANGES_SHOWN;
  return (
    <>
      <ul aria-label="What’s new" className={layout.list}>
        {(all ? changes : changes.slice(0, CHANGES_SHOWN)).map((change, index) => (
          // Published lines may repeat; their position identifies them.
          // biome-ignore lint/suspicious/noArrayIndexKey: a static, ordered list
          <li key={index}>{change}</li>
        ))}
      </ul>
      {hidden > 0 && (
        <div>
          <Button variant="quiet" aria-expanded={all} onPress={() => setAll(!all)}>
            {all ? "Show fewer changes" : `Show all ${changes.length} changes`}
          </Button>
        </div>
      )}
    </>
  );
}
