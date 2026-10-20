import { useNavigate } from "react-router";
import { Button } from "@/components/Button";
import { Page, PageHeader } from "@/components/Page";
import { useHasCapability } from "@/lib/commands";
import { PatientDirectory } from "./PatientDirectory";

/** Audited patient directory; identifying search criteria remain in memory. */
export function PatientsPage() {
  const navigate = useNavigate();
  const canEnterOrders = useHasCapability("enter_orders");
  return (
    <Page layout="collection">
      <PageHeader
        title="Patients"
        actions={
          canEnterOrders && (
            <Button variant="primary" onPress={() => navigate("/patients/register")}>
              Register patient
            </Button>
          )
        }
      />
      <PatientDirectory />
    </Page>
  );
}
