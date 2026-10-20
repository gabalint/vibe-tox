import { useState } from "react";
import { useWatch } from "react-hook-form";
import { Button } from "@/components/Button";
import { DataGrid } from "@/components/DataGrid";
import { CheckList, DATE_FORMAT, RecordPickerInput, TextInput } from "@/components/FormFields";
import { QueryContent } from "@/components/QueryContent";
import { RefreshControl } from "@/components/RefreshControl";
import { Section } from "@/components/Section";
import type { RegisterStandingOrderInput } from "@/generated/RegisterStandingOrderInput";
import type { StandingOrderView } from "@/generated/StandingOrderView";
import { CommandDialog } from "@/lib/CommandDialog";
import { useCommandQuery, useHasCapability } from "@/lib/commands";
import { OrderReferencesGate } from "./OrderReferencesGate";
import { type OrderReferences, practitionerOption } from "./references";

const VALIDITY =
  "Validity includes both boundary dates and is checked against the supplied collection date, using the original offset for a timestamp. Unknown collection dates hold testing and release. Delayed intake does not expire valid earlier collection. Accepted orders reserve a use; cancellation does not replenish it.";

export function StandingOrders({ subjectId }: { subjectId: string }) {
  const query = useCommandQuery<StandingOrderView[]>("list_standing_orders", {
    input: { subjectId },
  });
  const canEdit = useHasCapability("enter_orders");
  const [registering, setRegistering] = useState(false);
  return (
    <Section
      title="Patient-specific standing orders"
      actions={
        canEdit && <Button onPress={() => setRegistering(true)}>Record standing authority</Button>
      }
    >
      <p>{VALIDITY} A practice template alone does not authorize testing.</p>
      <RefreshControl query={query} />
      <OrderReferencesGate>
        {(references) => (
          <>
            <QueryContent query={query} what="standing orders">
              {(rows) => (
                <DataGrid
                  label="Standing authority history"
                  rows={rows}
                  getRowId={(r) => r.standingOrderId}
                  columns={[
                    { id: "document", header: "Source document", cell: (r) => r.documentRef },
                    {
                      id: "practitioner",
                      header: "Ordering practitioner",
                      cell: (r) => {
                        const p = references.practitioners.find((p) => p.id === r.practitionerId);
                        return p
                          ? `${p.familyName}, ${p.givenNames ?? ""} · NPI ${p.npi}`
                          : "Unavailable practitioner";
                      },
                    },
                    {
                      id: "validity",
                      header: "Collection dates",
                      cell: (r) => `${r.validFrom} through ${r.validUntil}`,
                    },
                    {
                      id: "uses",
                      header: "Accepted uses / limit",
                      cell: (r) => `${r.acceptedUses} / ${r.visitLimit ?? "No limit supplied"}`,
                    },
                    {
                      id: "scope",
                      header: "Authorized tests",
                      cell: (r) =>
                        r.scopeOrderableIds
                          .map((id) => {
                            const o = references.orderables.find((v) => v.orderableId === id);
                            return o ? `${o.code} ${o.name}` : "Previously offered test";
                          })
                          .join(", "),
                    },
                  ]}
                />
              )}
            </QueryContent>
            {registering && (
              <RegisterStanding
                subjectId={subjectId}
                references={references}
                onClose={() => setRegistering(false)}
              />
            )}
          </>
        )}
      </OrderReferencesGate>
    </Section>
  );
}

function RegisterStanding({
  subjectId,
  references,
  onClose,
}: {
  subjectId: string;
  references: OrderReferences;
  onClose: () => void;
}) {
  return (
    <CommandDialog<
      {
        practitionerId: string;
        scopeOrderableIds: string[];
        validFrom: string;
        validUntil: string;
        visitLimit: string;
        documentRef: string;
      },
      RegisterStandingOrderInput
    >
      title="Record patient-specific standing authority"
      submitLabel="Record standing authority"
      command={{ name: "register_standing_order", invalidates: ["list_standing_orders"] }}
      defaultValues={{
        practitionerId: "",
        scopeOrderableIds: [],
        validFrom: "",
        validUntil: "",
        visitLimit: "",
        documentRef: "",
      }}
      toInput={(v) => ({
        subjectId,
        practitionerId: v.practitionerId,
        scopeOrderableIds: v.scopeOrderableIds,
        validFrom: v.validFrom.trim(),
        validUntil: v.validUntil.trim(),
        visitLimit: v.visitLimit.trim() ? Number(v.visitLimit) : null,
        documentRef: v.documentRef.trim(),
      })}
      successTitle="Standing authority recorded"
      onClose={onClose}
    >
      <p>
        Record only authority supplied by this patient's ordering practitioner. The record is
        immutable; a later authorization is recorded separately.
      </p>
      <RecordPickerInput
        name="practitionerId"
        label="Ordering practitioner"
        options={references.practitioners.filter((p) => p.active).map(practitionerOption)}
        isRequired
      />
      <CheckList
        name="scopeOrderableIds"
        legend="Authorized tests"
        options={references.orderables.map((o) => ({
          id: o.orderableId,
          label: `${o.code} ${o.name}`,
        }))}
        requiredMessage="Choose the supplied authorized scope."
      />
      <TextInput
        name="validFrom"
        label="First valid collection date"
        format={DATE_FORMAT}
        isRequired
      />
      <TextInput
        name="validUntil"
        label="Last valid collection date"
        format={DATE_FORMAT}
        isRequired
      />
      <TextInput
        name="visitLimit"
        label="Visit limit"
        validate={(value) =>
          (/^[1-9]\d*$/.test(value) &&
            Number.isSafeInteger(Number(value)) &&
            Number(value) <= 4294967295) ||
          "Enter a whole number from 1 to 4294967295, or leave blank."
        }
        description="Positive whole number if supplied; blank means no visit limit supplied. One accepted order uses one visit."
      />
      <TextInput name="documentRef" label="Source approval reference" isRequired />
      <p>{VALIDITY}</p>
    </CommandDialog>
  );
}

export function StandingOrderChoice({ subjectId }: { subjectId: string }) {
  const practitionerId = useWatch({ name: "practitionerId" }) as string | null;
  const query = useCommandQuery<StandingOrderView[]>("list_standing_orders", {
    input: { subjectId },
  });
  return (
    <Section title="Standing authority, if supplied">
      <p>
        Optional. Select the patient's explicit authorization; this does not select tests or replace
        the supplied requisition. A template is separate.
      </p>
      <QueryContent query={query} what="standing authority">
        {(rows) => (
          <RecordPickerInput
            name="standingOrderId"
            label="Patient-specific standing order"
            options={rows
              .filter((r) => r.practitionerId === practitionerId)
              .map((r) => ({
                id: r.standingOrderId,
                label: r.documentRef,
                description: `${r.validFrom}–${r.validUntil}; ${r.acceptedUses} accepted uses${r.visitLimit === null ? "" : ` of ${r.visitLimit}`}`,
              }))}
            isDisabled={!practitionerId}
            description={
              practitionerId ? VALIDITY : "Choose the supplied ordering practitioner first."
            }
          />
        )}
      </QueryContent>
    </Section>
  );
}
