import { useState } from "react";
import { Button } from "@/components/Button";
import { FindField } from "@/components/FindField";
import { Toolbar } from "@/components/Toolbar";
import type { SubjectSearch } from "@/generated/SubjectSearch";
import type { SubjectSummary } from "@/generated/SubjectSummary";
import { PatientSearch, SEARCH_LIMIT } from "./PatientSearch";
import { SearchResults } from "./SearchResults";

const ALL_PATIENTS: SubjectSearch = { limit: SEARCH_LIMIT, offset: 0 };

/**
 * Patient directory (REQ-IDN-001), for people with `enter_orders` or `manage_quality`. Browsing and searching
 * are audited. Identifying search text stays in memory, out of URLs and browser storage.
 */
interface PatientDirectoryProps {
  onChoose?: (subject: SubjectSummary) => void;
  registrationPath?: string;
}

export function PatientDirectory({ onChoose, registrationPath }: PatientDirectoryProps) {
  const [criteria, setCriteria] = useState<SubjectSearch>(ALL_PATIENTS);
  const [formVersion, setFormVersion] = useState(0);
  const [findVersion, setFindVersion] = useState(0);
  const detailed = Boolean(criteria.identifier || criteria.familyName);
  const filtered = detailed || Boolean(criteria.query?.trim());
  const clear = () => {
    setCriteria(ALL_PATIENTS);
    setFormVersion((version) => version + 1);
    setFindVersion((version) => version + 1);
  };
  return (
    <SearchResults
      onChoose={onChoose}
      registrationPath={registrationPath}
      toolbar={(count) => (
        <Toolbar
          variant="collection"
          count={count}
          actions={filtered && <Button onPress={clear}>Show all patients</Button>}
          moreFilters={{
            applied: detailed ? "Detailed search applied" : undefined,
            content: (
              <PatientSearch
                key={formVersion}
                onSearch={(input) => {
                  setCriteria({ ...input, offset: 0 });
                  setFindVersion((version) => version + 1);
                }}
              />
            ),
          }}
        >
          <FindField
            key={findVersion}
            label="Find by name or identifier"
            placeholder="Name or identifier"
            value={criteria.query ?? ""}
            onChange={(query) => setCriteria({ ...ALL_PATIENTS, query })}
          />
        </Toolbar>
      )}
      criteria={criteria}
      filtered={filtered}
      onClear={clear}
      onPage={(offset) => setCriteria((current) => ({ ...current, offset }))}
    />
  );
}
