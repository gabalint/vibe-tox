import { useForm } from "react-hook-form";
import { Dialog } from "@/components/Dialog";
import { ErrorState } from "@/components/ErrorState";
import { LoadingState } from "@/components/LoadingState";
import type { CatalogVersionRef } from "@/generated/CatalogVersionRef";
import type { DraftOrderableVersion } from "@/generated/DraftOrderableVersion";
import type { MethodView } from "@/generated/MethodView";
import type { OrderableVersionView } from "@/generated/OrderableVersionView";
import type { UpdateOrderableDraft } from "@/generated/UpdateOrderableDraft";
import { useCommandQuery } from "@/lib/commands";
import { LIST_METHODS, ORDERABLE_QUERIES } from "../contract";
import { DraftDialog, type DraftMode, draftBase } from "../DraftDialog";
import { ComponentFields } from "./ComponentFields";
import { type OrderableFormValues, orderableValues, toOrderableContent } from "./orderableValues";

interface OrderableDraftDialogProps {
  orderableId: string;
  mode: DraftMode<OrderableVersionView>;
  onSaved: (draft: CatalogVersionRef) => void;
  onClose: () => void;
}

/**
 * Drafts a test or panel version or edits an open draft (REQ-ANA-009): its components,
 * each a current approved method version with its purpose and the analytes it reports.
 * The methods load first, so a copied version's components show their method.
 */
export function OrderableDraftDialog(props: OrderableDraftDialogProps) {
  const methods = useCommandQuery<MethodView[]>(LIST_METHODS);
  if (methods.data) return <OrderableDraftForm {...props} methods={methods.data} />;
  return (
    <Dialog
      title="Loading methods"
      isOpen
      onOpenChange={(isOpen) => {
        if (!isOpen) props.onClose();
      }}
    >
      {methods.isError ? (
        <ErrorState title="The methods could not be loaded" error={methods.error} />
      ) : (
        <LoadingState label="Loading methods…" />
      )}
    </Dialog>
  );
}

function OrderableDraftForm({
  orderableId,
  mode,
  onSaved,
  onClose,
  methods,
}: OrderableDraftDialogProps & { methods: readonly MethodView[] }) {
  const form = useForm<OrderableFormValues>({
    defaultValues: orderableValues(draftBase(mode), methods),
  });
  return (
    <DraftDialog<
      OrderableFormValues,
      OrderableVersionView,
      DraftOrderableVersion,
      UpdateOrderableDraft
    >
      mode={mode}
      commands={{ draft: "draft_orderable_version", update: "update_orderable_draft" }}
      invalidates={ORDERABLE_QUERIES}
      form={form}
      newInput={(values) => ({ orderableId, content: toOrderableContent(values) })}
      editInput={(draft, values) => ({
        orderableVersionId: draft.id,
        expectedRevision: draft.revision,
        content: toOrderableContent(values),
      })}
      onSaved={onSaved}
      onClose={onClose}
    >
      <ComponentFields methods={methods} />
    </DraftDialog>
  );
}
