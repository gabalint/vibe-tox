import { useRef, useState } from "react";
import { FormProvider, useForm } from "react-hook-form";
import { Navigate, useNavigate } from "react-router";
import { Button } from "@/components/Button";
import { Form } from "@/components/Form";
import { useToast } from "@/components/Toast";
import type { OrderDetail } from "@/generated/OrderDetail";
import type { RegisterOrderInput } from "@/generated/RegisterOrderInput";
import { useCommandMutation, useHasCapability } from "@/lib/commands";
import { UnsavedChangesGuard } from "@/lib/UnsavedChangesGuard";
import { LIST_ORDERS, REGISTER_ORDER } from "./contract";
import { EvidenceFields } from "./EvidenceFields";
import styles from "./OrderForm.module.css";
import { orderPath } from "./orderColumns";
import { EMPTY_ORDER, ORDER_FIELD_PATHS, toRegisterOrderInput } from "./orderValues";
import { RequisitionFields } from "./RequisitionFields";
import type { OrderReferences } from "./references";
import { StandingOrderChoice } from "./StandingOrders";
import { TestSelection } from "./TestSelection";

interface OrderFormProps {
  subjectId: string;
  references: OrderReferences;
}

/**
 * Records a requisition as received; the order then waits for a decision (REQ-ORD-001).
 * Leaving with typed input asks first; the saved order opens.
 */
export function OrderForm({ subjectId, references }: OrderFormProps) {
  const navigate = useNavigate();
  const toast = useToast();
  const form = useForm({ defaultValues: EMPTY_ORDER });
  const [savedPath, setSavedPath] = useState<string | null>(null);
  const receive = useRef(false);
  const canReceive = useHasCapability("accession");
  const register = useCommandMutation<OrderDetail, { input: RegisterOrderInput }>(REGISTER_ORDER, {
    invalidates: [LIST_ORDERS],
  });
  const onSubmit = form.handleSubmit((values) =>
    register.mutate(
      { input: toRegisterOrderInput(subjectId, values) },
      {
        onSuccess: (order) => {
          toast.show({ tone: "success", title: "Order received" });
          setSavedPath(`${orderPath(order.orderId)}${receive.current ? "?next=receive" : ""}`);
        },
      },
    ),
  );
  return (
    <FormProvider {...form}>
      {/* Before the redirect, so the guard is released when the saved order opens. */}
      <UnsavedChangesGuard when={form.formState.isDirty && savedPath === null} />
      {savedPath !== null && <Navigate to={savedPath} />}
      <div className={styles.entry}>
        <Form
          aria-label="New order"
          stickyActions
          onSubmit={onSubmit}
          error={register.error}
          errorTitle="The order was not saved"
          fieldPaths={ORDER_FIELD_PATHS}
          actions={
            <>
              <Button
                type="submit"
                variant={canReceive ? "secondary" : "primary"}
                isPending={register.isPending}
                onPress={() => {
                  receive.current = false;
                }}
              >
                Save order
              </Button>
              {canReceive && (
                <Button
                  type="submit"
                  variant="primary"
                  isPending={register.isPending}
                  onPress={() => {
                    receive.current = true;
                  }}
                >
                  Review and receive specimens
                </Button>
              )}
              <Button isDisabled={register.isPending} onPress={() => navigate(-1)}>
                Cancel
              </Button>
            </>
          }
        >
          <RequisitionFields
            clients={references.clients
              .filter((c) => c.active)
              .map((c) => ({ ...c, locations: c.locations.filter((l) => l.active) }))}
            practitioners={references.practitioners.filter((p) => p.active)}
          />
          <StandingOrderChoice subjectId={subjectId} />
          <TestSelection orderables={references.orderables} />
          <EvidenceFields />
          <p className={styles.hint}>
            Saving records the request. Order acceptance and specimen receipt are separate steps.
          </p>
        </Form>
      </div>
    </FormProvider>
  );
}
