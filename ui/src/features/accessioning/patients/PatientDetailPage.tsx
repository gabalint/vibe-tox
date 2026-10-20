import { useMemo } from "react";
import { useNavigate, useParams } from "react-router";
import { Button } from "@/components/Button";
import { DataGrid, type DataGridColumn } from "@/components/DataGrid";
import { Page, PageHeader } from "@/components/Page";
import { QueryContent } from "@/components/QueryContent";
import { RefreshControl } from "@/components/RefreshControl";
import { Section } from "@/components/Section";
import { Timestamp } from "@/components/Timestamp";
import type { IdentityAssertionView } from "@/generated/IdentityAssertionView";
import type { SubjectView } from "@/generated/SubjectView";
import { useCommandQuery, useHasCapability } from "@/lib/commands";
import { useFormatBirthDate } from "@/lib/format";
import { IdentityBanner, subjectSummary } from "@/lib/IdentityBanner";
import { useRecordSequence } from "@/lib/recordSequence";
import styles from "../accessioning.module.css";
import { PatientBillingSection } from "../billing/PatientBillingSection";
import { GET_SUBJECT } from "../contract";
import { StandingOrders } from "../orders/StandingOrders";
import { newOrderPath, orNotRecorded, patientPath, subjectName } from "../subjects";
import { MANAGE_QUALITY } from "./contract";
import { IdentityHolds } from "./IdentityHolds";
import { PatientMaintenance } from "./PatientMaintenance";

const PATIENTS = { label: "Patients", href: "/patients" };

function assertionColumns(
  formatBirthDate: (birthDate: string) => string,
): DataGridColumn<IdentityAssertionView>[] {
  return [
    {
      id: "assertedAt",
      header: "Recorded",
      cell: (each) => <Timestamp value={each.assertedAt} />,
      width: 190,
    },
    { id: "name", header: "Name", cell: subjectName },
    {
      id: "birthDate",
      header: "Birth date",
      cell: (each) => formatBirthDate(each.birthDate),
      width: 170,
    },
    {
      id: "sex",
      header: "Administrative sex",
      cell: (each) => orNotRecorded(each.administrativeSex),
      width: 160,
    },
    { id: "source", header: "Source", cell: (each) => each.source },
    { id: "reason", header: "Reason for change", cell: (each) => each.reason ?? "First record" },
  ];
}

const getAssertionId = (assertion: IdentityAssertionView) => assertion.id;

/**
 * One patient: identifiers, open identity holds and demographic history (REQ-IDN-001). The
 * header stays in place while another patient loads, so focus stays on Previous or Next.
 */
export function PatientDetailPage() {
  const { subjectId = "" } = useParams();
  const navigate = useNavigate();
  const canEnterOrders = useHasCapability("enter_orders");
  const subject = useCommandQuery<SubjectView>(GET_SUBJECT, { input: { subjectId } });
  const { list, sequence } = useRecordSequence(subjectId, PATIENTS, patientPath);
  const summary = subject.data && subjectSummary(subject.data);
  const name = summary ? subjectName(summary) : "Patient";
  return (
    <Page>
      <PageHeader
        title={name}
        breadcrumbs={[list, { label: name, href: patientPath(subjectId) }]}
        sequence={sequence}
        description="Viewing this patient is recorded in the audit trail."
        actions={
          summary &&
          canEnterOrders && (
            <>
              <Button onPress={() => navigate(`/orders?subjectId=${subjectId}`)}>Orders</Button>
              <Button variant="primary" onPress={() => navigate(newOrderPath(subjectId))}>
                New order
              </Button>
            </>
          )
        }
      />
      <RefreshControl query={subject} />
      {summary && <IdentityBanner subject={summary} />}
      <QueryContent query={subject} what="patient">
        {(data) => <PatientDetail subject={data} />}
      </QueryContent>
    </Page>
  );
}

function PatientDetail({ subject }: { subject: SubjectView }) {
  const canResolve = useHasCapability(MANAGE_QUALITY);
  const canEnterOrders = useHasCapability("enter_orders");
  const canBill = useHasCapability("billing");
  const formatBirthDate = useFormatBirthDate();
  const columns = useMemo(() => assertionColumns(formatBirthDate), [formatBirthDate]);
  return (
    <>
      <IdentityHolds subjectId={subject.id} holds={subject.openHolds} canResolve={canResolve} />
      <PatientMaintenance subject={subject} />
      {canEnterOrders && <StandingOrders subjectId={subject.id} />}
      {(canEnterOrders || canBill) && (
        <PatientBillingSection key={subject.id} subjectId={subject.id} />
      )}
      <Section title="Demographic history">
        <p className={styles.hint}>
          Details as each source gave them, oldest first. Earlier entries are never changed.
        </p>
        <DataGrid
          label="Demographic history"
          columns={columns}
          rows={subject.assertions}
          getRowId={getAssertionId}
        />
      </Section>
    </>
  );
}
