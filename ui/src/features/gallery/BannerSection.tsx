import { Banner } from "@/components/Banner";
import { StatusBadge } from "@/components/StatusBadge";
import type { SubjectSummary } from "@/generated/SubjectSummary";
import { IdentityBanner } from "@/lib/IdentityBanner";
import { GallerySection } from "./GallerySection";
import styles from "./gallery.module.css";

/** A synthetic infant patient with an open identity hold; no real person. */
const SUBJECT: SubjectSummary = {
  subjectId: "00000000-0000-7000-8000-00000000a001",
  revision: 1,
  familyName: "Synthetic-Example",
  givenNames: "Avery Jo",
  birthDate: "2026-02-01",
  administrativeSex: null,
  identifiers: [
    {
      id: "00000000-0000-7000-8000-00000000a002",
      assigningAuthority: "SYN-CLINIC",
      identifierType: "MR",
      value: "MR-000123",
      active: true,
      addedAt: "2026-10-12T14:00:00Z",
      deactivatedAt: null,
      deactivationReason: null,
    },
  ],
  openIdentityHolds: 1,
};

/** The sticky strip of facts, and the identity banner built on it. */
export function BannerSection() {
  return (
    <GallerySection title="Banners">
      <div className={styles.stack}>
        <Banner
          label="Synthetic batch"
          items={[
            { label: "Batch", value: "SYN-B-000042", emphasis: true },
            { label: "Positions", value: "12" },
          ]}
          status={<StatusBadge tone="info">Queued</StatusBadge>}
        />
        <IdentityBanner
          subject={SUBJECT}
          specimen={{ accessionNumber: "SYN-ACC-000777", labelCode: "SYN-LBL-0001" }}
        />
      </div>
    </GallerySection>
  );
}
