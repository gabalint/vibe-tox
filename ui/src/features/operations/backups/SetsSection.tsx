import { useState } from "react";
import { Button } from "@/components/Button";
import { QueryContent } from "@/components/QueryContent";
import { Timestamp } from "@/components/Timestamp";
import type { BackupSetView } from "@/generated/BackupSetView";
import type { TargetInput } from "@/generated/TargetInput";
import type { TargetView } from "@/generated/TargetView";
import { useCommandQuery } from "@/lib/commands";
import styles from "@/styles/layout.module.css";
import { LIST_BACKUP_SETS } from "../contract";
import { backupKind, sizeText } from "../labels";

/** A small child list: the newest sets, with the rest behind a button. */
const PREVIEW = 10;

function setText(set: BackupSetView): string {
  const base = `${backupKind(set.kind)}, app ${set.appVersion}, ${sizeText(set.databaseBytes)}`;
  if (set.compatible) return base;
  return `${base}, not compatible${set.incompatibleReason ? `: ${set.incompatibleReason}` : ""}`;
}

/**
 * The complete sets on one target as the engine reads them from their manifests, newest first.
 * The engine lists them for active targets only.
 */
export function SetsSection({ target }: { target: TargetView }) {
  const input: TargetInput = { targetId: target.targetId };
  const sets = useCommandQuery<BackupSetView[]>(LIST_BACKUP_SETS, { input });
  const [showAll, setShowAll] = useState(false);
  return (
    <div className={styles.stack}>
      <h3 className={styles.subheading}>Backup sets on this target</h3>
      <QueryContent
        query={sets}
        what="backup sets"
        empty={{
          when: (rows) => rows.length === 0,
          title: "No sets on this target",
          description: "No complete backup was found on this target.",
        }}
      >
        {(rows) => {
          // The engine lists the oldest first.
          const newest = rows.toReversed();
          const shown = showAll ? newest : newest.slice(0, PREVIEW);
          return (
            <>
              <ul aria-label={`Sets on ${target.label}`} className={styles.list}>
                {shown.map((set) => (
                  <li key={set.setId}>
                    <Timestamp value={set.createdAt} />, {setText(set)}
                  </li>
                ))}
              </ul>
              {newest.length > PREVIEW && (
                <Button onPress={() => setShowAll(!showAll)}>
                  {showAll ? `Show the newest ${PREVIEW}` : `Show all ${newest.length} sets`}
                </Button>
              )}
            </>
          );
        }}
      </QueryContent>
    </div>
  );
}
