import { useState } from "react";
import { FormProvider, useForm } from "react-hook-form";
import { Navigate, useNavigate } from "react-router";
import { Button } from "@/components/Button";
import { EmptyState } from "@/components/EmptyState";
import { Form } from "@/components/Form";
import { Section } from "@/components/Section";
import { useToast } from "@/components/Toast";
import type { AccessionDetail } from "@/generated/AccessionDetail";
import type { CreateAccessionInput } from "@/generated/CreateAccessionInput";
import type { OrderAccessionContext } from "@/generated/OrderAccessionContext";
import type { OrderDetail } from "@/generated/OrderDetail";
import type { SpecimenTypeView } from "@/generated/SpecimenTypeView";
import { useCommandMutation } from "@/lib/commands";
import { UnsavedChangesGuard } from "@/lib/UnsavedChangesGuard";
import { GET_ORDER, LIST_ORDERS } from "../orders/contract";
import { ORDER_ACCESSION_CONTEXT } from "../useOrderAccessionContext";
import { AccessionTestSelection } from "./AccessionTestSelection";
import { EMPTY_ACCESSION, toCreateAccessionInput } from "./accessionValues";
import { CollectionFields } from "./CollectionFields";
import { CREATE_ACCESSION } from "./contract";
import styles from "./receiving.module.css";
import { SpecimenList } from "./SpecimenList";

interface AccessionFormProps {
  order: OrderDetail;
  context: OrderAccessionContext;
  specimenTypes: readonly SpecimenTypeView[];
}

/** A failed or stale command keeps every entered field available for correction. */
export function AccessionForm({ order, context, specimenTypes }: AccessionFormProps) {
  const navigate = useNavigate();
  const toast = useToast();
  const form = useForm({ defaultValues: EMPTY_ACCESSION });
  const [savedId, setSavedId] = useState<string | null>(null);
  const create = useCommandMutation<AccessionDetail, { input: CreateAccessionInput }>(
    CREATE_ACCESSION,
    { invalidates: [GET_ORDER, LIST_ORDERS, ORDER_ACCESSION_CONTEXT] },
  );
  const onSubmit = form.handleSubmit((values) =>
    create.mutate(
      { input: toCreateAccessionInput(order.orderId, values) },
      {
        onSuccess: (accession) => {
          toast.show({
            tone: "success",
            title: `Accession ${accession.accessionNumber} created`,
            description: "Check the container labels and record each receipt decision.",
          });
          setSavedId(accession.accessionId);
        },
      },
    ),
  );
  // Keep this component mounted while mutation invalidation updates eligibility, so its
  // success callback can release the unsaved-input guard and open the created accession.
  const canReceive =
    context.allowedActions.includes("create_accession") &&
    context.items.some((item) => item.eligible);
  const blocked = !canReceive && !create.isPending;
  return (
    <FormProvider {...form}>
      <UnsavedChangesGuard when={form.formState.isDirty && savedId === null} />
      {savedId !== null && <Navigate to={`/accessions/${savedId}`} />}
      {blocked ? (
        <>
          <EmptyState
            title="No tests available to receive"
            description="Review this order's test status or continue an existing accession below."
          />
          <AccessionTestSelection order={order} context={context} />
        </>
      ) : (
        <Form
          aria-label="Receive specimens"
          stickyActions
          fieldPaths={{
            "collection.time.timeZone": "timeZone",
            "collection.utcOffsetMinutes": "utcOffset",
            "collection.date": "collectedDate",
            "collection.time": "collectedTime",
            "collection.collector": "collector",
          }}
          onSubmit={onSubmit}
          error={create.error}
          errorTitle="The specimens were not saved"
          actions={
            <>
              <Button type="submit" variant="primary" isPending={create.isPending}>
                Receive specimens
              </Button>
              <Button
                isDisabled={create.isPending}
                onPress={() => navigate(`/orders/${order.orderId}`)}
              >
                Cancel
              </Button>
            </>
          }
        >
          <AccessionTestSelection order={order} context={context} selectable />
          <div className={styles.fields}>
            <CollectionFields />
            <Section title="Material received" variant="plain">
              <SpecimenList specimenTypes={specimenTypes} />
            </Section>
          </div>
        </Form>
      )}
    </FormProvider>
  );
}
