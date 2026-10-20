import { useMemo } from "react";
import { Checkbox } from "@/components/Checkbox";
import { RecordPicker } from "@/components/RecordPicker";
import { Select } from "@/components/Select";
import type { ClientView } from "@/generated/ClientView";
import { choiceOf } from "@/lib/choices";
import { useSearchParam } from "@/lib/searchParam";
import { type DirectoryFilters, useClearParams } from "../Directory";
import type { DestinationView } from "./contract";
import { STATE_FILTERS } from "./labels";

/** The same wording and `?retired=show` as every list that hides retired records. */
const RETIRED_PARAM = "retired";
const SHOW = "show";
const PARAMS = ["client", "state", RETIRED_PARAM];

/**
 * The directory's filters (client, state and "Show retired"), kept in the address with the
 * find text and the open record. Changing one replaces the history entry and keeps the other
 * parameters. Retired destinations are out of the count until shown. The engine returns the
 * whole list (NFR-33 bound).
 */
export function useDestinationFilters(
  clients: readonly ClientView[],
): DirectoryFilters<DestinationView> {
  const [clientId, setClientId] = useSearchParam("client");
  const [stateParam, setState] = useSearchParam("state");
  const [retired, setRetired] = useSearchParam(RETIRED_PARAM);
  const clear = useClearParams(PARAMS);
  const state = choiceOf(STATE_FILTERS, stateParam) ?? "all";
  const showRetired = retired === SHOW;
  const options = useMemo(
    () =>
      clients.map((client) => ({ id: client.id, label: client.name, description: client.code })),
    [clients],
  );
  const scope = useMemo(
    () => (destination: DestinationView) => showRetired || !destination.retired,
    [showRetired],
  );
  const keep = useMemo(
    () => (destination: DestinationView) =>
      (clientId === null || destination.clientId === clientId) &&
      (state === "all" || destination.state === state),
    [clientId, state],
  );
  return {
    controls: (
      <>
        <RecordPicker
          layout="inline"
          label="Client"
          name="client"
          options={options}
          value={clientId}
          onChange={setClientId}
        />
        <Select
          layout="inline"
          label="State"
          name="state"
          options={STATE_FILTERS}
          value={state}
          onChange={(next) => {
            const chosen = choiceOf(STATE_FILTERS, next) ?? "all";
            setState(chosen === "all" ? null : chosen);
          }}
        />
      </>
    ),
    moreFilters: {
      content: (
        <Checkbox isSelected={showRetired} onChange={(show) => setRetired(show ? SHOW : null)}>
          Show retired
        </Checkbox>
      ),
      applied: showRetired ? "Retired destinations included" : undefined,
    },
    scopeLabel: showRetired ? undefined : "retired excluded",
    isSet: clientId !== null || state !== "all" || showRetired,
    scope,
    keep,
    clear,
  };
}
