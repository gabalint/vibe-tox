import { Page, PageHeader } from "@/components/Page";
import { ActionsSection } from "./ActionsSection";
import { BannerSection } from "./BannerSection";
import { DataStateSection } from "./DataStateSection";
import { FeedbackSection } from "./FeedbackSection";
import { FieldsSection } from "./FieldsSection";
import { FindSection } from "./FindSection";
import { FormPartsSection } from "./FormPartsSection";
import { GridDemo } from "./GridDemo";
import { NavigationSection } from "./NavigationSection";
import { PickSection } from "./PickSection";
import { RecordViewsSection } from "./RecordViewsSection";

/** Development builds only: every design-system component and variant, plus the grid demo. */
export function ComponentGallery() {
  return (
    <main>
      <Page>
        <PageHeader
          title="Component gallery"
          description="Design-system components and their variants. Development builds only; synthetic data."
        />
        <ActionsSection />
        <FieldsSection />
        <FormPartsSection />
        <FeedbackSection />
        <DataStateSection />
        <NavigationSection />
        <FindSection />
        <PickSection />
        <BannerSection />
        <RecordViewsSection />
        <GridDemo />
      </Page>
    </main>
  );
}
