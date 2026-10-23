import { useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { Button } from "@/components/Button";
import { QueryContent } from "@/components/QueryContent";
import { RefreshControl } from "@/components/RefreshControl";
import { Timestamp } from "@/components/Timestamp";
import type { MaterialHistoryEntry } from "@/generated/MaterialHistoryEntry";
import type { MaterialHistoryPage } from "@/generated/MaterialHistoryPage";
import type { MaterialHistorySnapshot } from "@/generated/MaterialHistorySnapshot";
import { useCommandQuery } from "@/lib/commands";
import { usePersonText } from "@/lib/person";
export function MaterialHistory({ containerId }: { containerId: string }) {
  const client = useQueryClient();
  const [cursor, setCursor] = useState<{
    offset: number;
    snapshot: MaterialHistorySnapshot | null;
  }>({ offset: 0, snapshot: null });
  const { offset, snapshot } = cursor;
  const query = useCommandQuery<MaterialHistoryPage>("get_container_history", {
    input: { containerId, offset, snapshot },
  });
  const refresh = () => {
    void client.invalidateQueries({ queryKey: ["get_container_history"], refetchType: "none" });
    if (offset === 0 && snapshot === null) return query.refetch();
    setCursor({ offset: 0, snapshot: null });
  };
  return (
    <>
      <h3>Material history</h3>
      <RefreshControl query={{ ...query, refetch: refresh }} />
      <QueryContent query={query} what="material history">
        {(page) => (
          <>
            <HistoryList title="Location moves" entries={page.moves} />
            <HistoryList title="Holds and releases" entries={page.holds} />
            <HistoryList
              title="Terminal material events"
              entries={[page.depletion, page.disposal].filter(
                (e): e is MaterialHistoryEntry => e !== null,
              )}
            />
            <p>
              Moves follow committed sequence where known. Other streams are displayed separately;
              recorded times do not establish a shared event order. Pages retain their membership
              while browsing; Refresh starts from the latest history.
            </p>
            <Button
              isDisabled={offset === 0}
              onPress={() =>
                setCursor({ offset: Math.max(0, offset - 50), snapshot: page.snapshot })
              }
            >
              Previous history
            </Button>
            <Button
              isDisabled={!page.hasMore}
              onPress={() => setCursor({ offset: offset + 50, snapshot: page.snapshot })}
            >
              Next history
            </Button>
          </>
        )}
      </QueryContent>
    </>
  );
}
function HistoryList({ title, entries }: { title: string; entries: MaterialHistoryEntry[] }) {
  const person = usePersonText();
  return (
    <section aria-label={title}>
      <h4>{title}</h4>
      {entries.length === 0 ? (
        <p>No events on this page.</p>
      ) : (
        <ul>
          {entries.map((e) => (
            <li key={e.id}>
              {e.description}. Recorded by {person(e.actor, e.actorDisplay)},{" "}
              <Timestamp value={e.recordedAt} />. {e.reason && `Reason: ${e.reason}.`}{" "}
              {e.kind === "move" &&
                (e.causalSequence === null
                  ? "Legacy chronology unresolved."
                  : `Committed sequence ${e.causalSequence}.`)}
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
