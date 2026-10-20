import { Link, useParams } from "react-router";
import { Button } from "@/components/Button";
import { Facts } from "@/components/Facts";
import { Page, PageHeader } from "@/components/Page";
import { QueryContent } from "@/components/QueryContent";
import { Section } from "@/components/Section";
import type { AccessionDetail } from "@/generated/AccessionDetail";
import { useCommandQuery, useHasCapability } from "@/lib/commands";
import { useFormatTime } from "@/lib/format";
import { IdentityBanner } from "@/lib/IdentityBanner";
import { useRecordSequence } from "@/lib/recordSequence";
import { CollectionHistorySection } from "../billing/CollectionHistorySection";
import { OrderBillingSection } from "../billing/OrderBillingSection";
import { IntakeSteps } from "../IntakeSteps";
import { orNotRecorded } from "../subjects";
import { AccessionProgress } from "./AccessionProgress";
import { GET_ACCESSION } from "./contract";
import { collectionTimeText } from "./labels";
import { SpecimenPanel } from "./SpecimenPanel";

/** Accessions have no list of their own; they are found by scanning a container. */
const CONTAINER_LOOKUP = { label: "Container lookup", href: "/containers" };

const accessionPath = (accessionId: string) => `/accessions/${accessionId}`;

/**
 * One accession: collection, specimens, containers and labels (REQ-SPC-001 to REQ-SPC-003).
 * Viewing it is an audited access, so label outcomes are refreshed on request, not polled.
 * The header stays in place while another accession loads, so focus stays on Previous or
 * Next.
 */
export function AccessionPage() {
  const { accessionId = "" } = useParams();
  const canOpenOrder = useHasCapability("enter_orders");
  const accession = useCommandQuery<AccessionDetail>(GET_ACCESSION, { input: { accessionId } });
  const { list, sequence } = useRecordSequence(accessionId, CONTAINER_LOOKUP, accessionPath);
  const data = accession.data;
  const title = data ? `Accession ${data.accessionNumber}` : "Accession";
  return (
    <Page>
      <PageHeader
        title={title}
        breadcrumbs={[
          list,
          ...(data && canOpenOrder ? [{ label: "Order", href: `/orders/${data.orderId}` }] : []),
          { label: title, href: accessionPath(accessionId) },
        ]}
        sequence={sequence}
        description="Collection, received material and receipt decisions for this accession."
        actions={
          data && (
            <Button isPending={accession.isFetching} onPress={() => void accession.refetch()}>
              Check label status
            </Button>
          )
        }
      />
      <IntakeSteps current="finish" />
      {data && (
        <IdentityBanner
          subjectId={data.subjectId}
          specimen={{ accessionNumber: data.accessionNumber }}
        />
      )}
      <QueryContent query={accession} what="accession">
        {(loaded) => <AccessionRecord accession={loaded} />}
      </QueryContent>
    </Page>
  );
}

function AccessionRecord({ accession }: { accession: AccessionDetail }) {
  const formatTime = useFormatTime();
  const orders = useHasCapability("enter_orders");
  const billing = useHasCapability("billing");
  const { accessionNumber, collection, specimens } = accession;
  return (
    <>
      <AccessionProgress accession={accession} />
      <Section
        title="Collection"
        actions={
          <Link to={`/accessions/${accession.accessionId}/collection-review`}>
            Collection timing review
          </Link>
        }
      >
        <Facts
          layout="compact"
          items={[
            { label: "Collected", value: collectionTimeText(collection, formatTime) },
            { label: "Collected by", value: orNotRecorded(collection.collector) },
          ]}
        />
        <CollectionHistorySection key={accession.accessionId} accessionId={accession.accessionId} />
      </Section>
      {(orders || billing) && (
        <OrderBillingSection
          key={accession.orderId}
          orderId={accession.orderId}
          subjectId={accession.subjectId}
        />
      )}
      {specimens.map((specimen, index) => (
        <SpecimenPanel
          key={specimen.specimenId}
          specimen={specimen}
          title={`Specimen ${index + 1} (${specimen.specimenTypeCode})`}
          accessionNumber={accessionNumber}
        />
      ))}
    </>
  );
}
