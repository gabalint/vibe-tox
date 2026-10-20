import { FormProvider, useForm } from "react-hook-form";
import { Link, useLocation, useSearchParams } from "react-router";
import { Button } from "@/components/Button";
import { EmptyState } from "@/components/EmptyState";
import { ErrorState } from "@/components/ErrorState";
import { Facts } from "@/components/Facts";
import { Form } from "@/components/Form";
import { TextInput } from "@/components/FormFields";
import { LoadingState } from "@/components/LoadingState";
import { Page, PageHeader } from "@/components/Page";
import { RefreshControl } from "@/components/RefreshControl";
import { Section } from "@/components/Section";
import { StatusBadge } from "@/components/StatusBadge";
import type { ContainerLookup } from "@/generated/ContainerLookup";
import { useCommandQuery } from "@/lib/commands";
import { IdentityBanner } from "@/lib/IdentityBanner";
import styles from "../accessioning.module.css";
import { ContainerMaterial } from "./ContainerMaterial";
import { HoldCount, LatestLabelJob } from "./containerColumns";
import { FIND_CONTAINER } from "./contract";
import { acceptanceStatus, containerStateLabel, volumeText } from "./labels";

/**
 * Finds a container by its barcode (REQ-SPC-001), for accession or quality staff. A scanner
 * types the barcode and presses Enter, here or in the top bar's scan field; the barcode goes
 * in the address (`?labelCode=`) and the field clears for the next scan. Each scan is its own
 * lookup, even of the same barcode: the new history entry remounts the result.
 */
export function ContainerLookupPage() {
  const [params, setParams] = useSearchParams();
  const { key } = useLocation();
  const labelCode = params.get("labelCode")?.trim() ?? "";
  const form = useForm({ defaultValues: { labelCode: "" } });
  const onSubmit = form.handleSubmit((values) => {
    setParams({ labelCode: values.labelCode.trim() });
    form.reset();
    form.setFocus("labelCode");
  });
  return (
    <Page>
      <PageHeader
        title="Container lookup"
        description="Scan or type a container barcode. Each lookup is recorded in the audit trail."
      />
      {labelCode !== "" && <ContainerBanner key={`banner-${key}`} labelCode={labelCode} />}
      <FormProvider {...form}>
        <Form
          aria-label="Container lookup"
          onSubmit={onSubmit}
          actions={
            <Button type="submit" variant="primary">
              Find container
            </Button>
          }
        >
          <TextInput name="labelCode" label="Container barcode" isRequired autoFocus />
        </Form>
      </FormProvider>
      {labelCode !== "" && <ContainerResult key={`result-${key}`} labelCode={labelCode} />}
    </Page>
  );
}

function useContainerLookup(labelCode: string) {
  return useCommandQuery<ContainerLookup>(FIND_CONTAINER, { input: { labelCode } });
}

/** Whose container it is, once found: the lookup already carries the patient. */
function ContainerBanner({ labelCode }: { labelCode: string }) {
  const match = useContainerLookup(labelCode);
  if (!match.isSuccess) return null;
  const { container, accessionNumber, subject } = match.data;
  return (
    <IdentityBanner
      subject={subject}
      specimen={{ accessionNumber, labelCode: container.labelCode }}
    />
  );
}

function ContainerResult({ labelCode }: { labelCode: string }) {
  const match = useContainerLookup(labelCode);
  if (match.isPending) return <LoadingState label={`Looking up ${labelCode}…`} />;
  if (match.error?.code === "not_found") {
    return (
      <EmptyState
        title={`No container has the barcode ${labelCode}`}
        description="Check the label and scan again."
      />
    );
  }
  if (match.isError) {
    return <ErrorState title={`${labelCode} could not be looked up`} error={match.error} />;
  }
  const { container, accessionId, accessionNumber } = match.data;
  const acceptance = acceptanceStatus(match.data.acceptance);
  return (
    <Section
      title={
        <>
          Container <span className={styles.code}>{container.labelCode}</span>
        </>
      }
    >
      <RefreshControl query={match} />
      <Facts
        layout="compact"
        items={[
          {
            label: "Accession",
            value: <Link to={`/accessions/${accessionId}`}>{accessionNumber}</Link>,
          },
          {
            label: "Specimen",
            value: (
              <>
                {match.data.specimenTypeCode}{" "}
                <StatusBadge tone={acceptance.tone}>{acceptance.label}</StatusBadge>
              </>
            ),
          },
          { label: "Kind", value: container.kind === "primary" ? "Primary" : "Aliquot" },
          { label: "State", value: containerStateLabel(container.state) },
          { label: "Volume", value: volumeText(container) },
          { label: "Location", value: container.location?.name ?? "Not recorded" },
          { label: "Holds", value: <HoldCount count={container.openHolds.length} /> },
          { label: "Label printing", value: <LatestLabelJob container={container} /> },
        ]}
      />
      <ContainerMaterial container={container} />
    </Section>
  );
}
