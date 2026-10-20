import { BookOpen, HardDrive, Users } from "lucide-react";
import { useContext } from "react";
import { DemoEntryContext } from "./demoSlot";
import styles from "./setup.module.css";

/** What the bundled demo contains, before a person chooses an account or data load. */
export function SetupIntroduction({ isDemo }: { isDemo: boolean }) {
  const DemoEntry = useContext(DemoEntryContext);
  return (
    <aside className={styles.introduction} aria-label="About your workspace">
      <div>
        <h2>
          {isDemo ? "A head start for exploring." : "Try a laboratory with data already in place."}
        </h2>
        <p className={styles.lead}>
          {isDemo
            ? "Create your account and we’ll prepare a workspace you can explore at your own pace."
            : "Get to know Vibe Tox with a separate demo laboratory, a starter catalog and fictional people."}
        </p>
      </div>
      {!isDemo && DemoEntry && <DemoEntry />}
      <dl className={styles.included}>
        <div>
          <dt>
            <BookOpen aria-hidden="true" />A catalog to start from
          </dt>
          <dd>Urine and oral-fluid tests, panels and methods, loaded as drafts for review.</dd>
        </div>
        <div>
          <dt>
            <Users aria-hidden="true" />
            People to practice with
          </dt>
          <dd>
            Explore fictional clinics, practitioners and patients, then review the catalog for a
            practice order.
          </dd>
        </div>
        <div>
          <dt>
            <HardDrive aria-hidden="true" />A separate space on this computer
          </dt>
          <dd>Demo data stays in its own laboratory. Leave or reset the demo whenever you need.</dd>
        </div>
      </dl>
      <p className={styles.note}>
        For evaluation with synthetic data only. The demo does not include completed orders or
        results, and is not validated for clinical use.
      </p>
    </aside>
  );
}
