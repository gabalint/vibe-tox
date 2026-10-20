import { useParams } from "react-router";
import { Button } from "@/components/Button";
import { Page, PageHeader } from "@/components/Page";
import { QueryContent } from "@/components/QueryContent";
import { Section } from "@/components/Section";
import type { CollectionHistory } from "@/generated/CollectionHistory";
import { useCommandQuery } from "@/lib/commands";
import { IdentityBanner } from "@/lib/IdentityBanner";
import { CollectionHistoryContent } from "./CollectionHistorySection";

/** Scoped quality access: no clinical accession or patient query is required. */
export function CollectionReviewPage() {
  const { accessionId = "" } = useParams();
  const query = useCommandQuery<CollectionHistory>("get_collection_history", {
    input: { accessionId },
  });
  return (
    <Page>
      <PageHeader
        title="Collection timing review"
        breadcrumbs={[
          { label: "Work queues", href: "/work-queues" },
          {
            label: "Collection timing review",
            href: `/accessions/${accessionId}/collection-review`,
          },
        ]}
        description="Review original collection information, corrections and quality decisions."
        actions={
          <Button isPending={query.isFetching} onPress={() => void query.refetch()}>
            Refresh collection history
          </Button>
        }
      />
      <QueryContent query={query} what="collection history">
        {(history) => (
          <>
            <IdentityBanner
              subject={history.subject}
              specimen={{ accessionNumber: history.accessionNumber }}
            />
            <Section title={`Accession ${history.accessionNumber}`}>
              <CollectionHistoryContent key={accessionId} history={history} />
            </Section>
          </>
        )}
      </QueryContent>
    </Page>
  );
}
