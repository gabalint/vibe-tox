import { Checkbox } from "@/components/Checkbox";
import type { CatalogPreviewBlankCutoff } from "@/generated/CatalogPreviewBlankCutoff";
import layout from "@/styles/layout.module.css";

export function cutoffKey(cutoff: CatalogPreviewBlankCutoff): string {
  return `${cutoff.methodVersionId} ${cutoff.analyteId}`;
}

interface BlankCutoffChecksProps {
  cutoffs: readonly CatalogPreviewBlankCutoff[];
  confirmed: ReadonlySet<string>;
  onChange: (confirmed: ReadonlySet<string>) => void;
}

/**
 * The blank cutoffs a Catalog Preview lists, each confirmed on its own as not applicable, with
 * "Confirm all listed" as a convenience; every one stays visible.
 */
export function BlankCutoffChecks({ cutoffs, confirmed, onChange }: BlankCutoffChecksProps) {
  if (cutoffs.length === 0) {
    return <p>The report lists no blank cutoffs.</p>;
  }
  const count = cutoffs.filter((cutoff) => confirmed.has(cutoffKey(cutoff))).length;
  const toggle = (key: string, on: boolean) => {
    const next = new Set(confirmed);
    if (on) next.add(key);
    else next.delete(key);
    onChange(next);
  };
  return (
    <fieldset className={layout.stack}>
      <legend className={layout.subheading}>
        Blank cutoffs to confirm as not applicable ({cutoffs.length})
      </legend>
      <p className={layout.hint}>
        A blank cutoff means the method reports no cutoff for that analyte. If one should have a
        cutoff, cancel and fill it in the method's draft first.
      </p>
      <Checkbox
        isSelected={count === cutoffs.length}
        isIndeterminate={count > 0 && count < cutoffs.length}
        onChange={(on) => onChange(new Set(on ? cutoffs.map(cutoffKey) : []))}
      >
        Confirm all listed
      </Checkbox>
      <ul className={layout.list}>
        {cutoffs.map((cutoff) => {
          const key = cutoffKey(cutoff);
          return (
            <li key={key}>
              <Checkbox isSelected={confirmed.has(key)} onChange={(on) => toggle(key, on)}>
                <span className={layout.code}>
                  {cutoff.methodCode} version {cutoff.methodVersion}: {cutoff.analyteCode}
                </span>
              </Checkbox>
            </li>
          );
        })}
      </ul>
    </fieldset>
  );
}
