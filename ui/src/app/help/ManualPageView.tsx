import type { ManualPage } from "./articles";
import styles from "./PageHelp.module.css";

/** Each entry uses a narrow, two-column reference table that also reads at minimum panel width. */
function ReferenceEntry({ name, rows }: { name: string; rows: [string, string][] }) {
  return (
    <details className={styles.entry}>
      <summary>{name}</summary>
      <table className={styles.reference} aria-label={name}>
        <tbody>
          {rows.map(([label, value]) => (
            <tr key={label}>
              <th scope="row">{label}</th>
              <td>{value}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </details>
  );
}

export function ManualPageView({ page }: { page: ManualPage }) {
  return (
    <div className={styles.manual}>
      <p>{page.purpose}</p>
      <p>
        <strong>Access and prerequisites. </strong>
        {page.access}
      </p>
      <details open className={styles.chapter}>
        <summary>Fields and displayed information ({page.fields.length})</summary>
        {page.fields.map((field) => (
          <ReferenceEntry
            key={field.name}
            name={field.name}
            rows={[
              ["What it is / used for", field.meaning],
              ["Possible values", field.values],
              ["Rules and limits", field.rules],
            ]}
          />
        ))}
      </details>
      <details open className={styles.chapter}>
        <summary>Actions ({page.actions.length})</summary>
        {page.actions.length === 0 && (
          <p>This page provides information and has no page-specific actions.</p>
        )}
        {page.actions.map((action) => (
          <ReferenceEntry
            key={action.name}
            name={action.name}
            rows={[
              ["What it does", action.does],
              ["Available when", action.availableWhen],
              ["Result / next step", action.result],
            ]}
          />
        ))}
      </details>
      <details className={styles.chapter}>
        <summary>Workflow</summary>
        <ol>
          {page.workflow.map((step) => (
            <li key={step}>{step}</li>
          ))}
        </ol>
      </details>
      <details className={styles.chapter}>
        <summary>Restrictions and limitations</summary>
        <ul>
          {page.restrictions.map((rule) => (
            <li key={rule}>{rule}</li>
          ))}
        </ul>
      </details>
      <details className={styles.chapter}>
        <summary>Troubleshooting</summary>
        {page.troubleshooting.map((entry) => (
          <ReferenceEntry
            key={entry.problem}
            name={entry.problem}
            rows={[["Recovery", entry.recovery]]}
          />
        ))}
      </details>
    </div>
  );
}
