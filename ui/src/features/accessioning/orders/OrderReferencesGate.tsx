import type { ReactNode } from "react";
import { ErrorState } from "@/components/ErrorState";
import { LoadingState } from "@/components/LoadingState";
import type { ClientView } from "@/generated/ClientView";
import type { OrderableVersionView } from "@/generated/OrderableVersionView";
import type { PractitionerView } from "@/generated/PractitionerView";
import { useCommandQuery } from "@/lib/commands";
import { LIST_ACTIVE_ORDERABLES, LIST_CLIENTS, LIST_PRACTITIONERS } from "./contract";
import type { OrderReferences } from "./references";

/** The clients, practitioners and current tests: `null` until all three have loaded. */
export function useOrderReferences() {
  const clients = useCommandQuery<ClientView[]>(LIST_CLIENTS);
  const practitioners = useCommandQuery<PractitionerView[]>(LIST_PRACTITIONERS);
  const orderables = useCommandQuery<OrderableVersionView[]>(LIST_ACTIVE_ORDERABLES);
  const failed = [clients, practitioners, orderables].find((query) => query.isError) ?? null;
  const references: OrderReferences | null =
    clients.data && practitioners.data && orderables.data
      ? { clients: clients.data, practitioners: practitioners.data, orderables: orderables.data }
      : null;
  return { references, failed };
}

/** Loads the clients, practitioners and current tests, then renders `children` with them. */
export function OrderReferencesGate({
  children,
}: {
  children: (references: OrderReferences) => ReactNode;
}) {
  const { references, failed } = useOrderReferences();
  if (failed?.error) {
    return (
      <ErrorState
        title="Clients, practitioners or tests could not be loaded"
        error={failed.error}
        onRetry={() => void failed.refetch()}
      />
    );
  }
  if (references === null) {
    return <LoadingState label="Loading clients, practitioners and tests…" />;
  }
  return children(references);
}
