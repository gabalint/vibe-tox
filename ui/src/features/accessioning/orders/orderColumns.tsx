import type { DataGridColumn } from "@/components/DataGrid";
import { StatusBadge } from "@/components/StatusBadge";
import { Timestamp } from "@/components/Timestamp";
import type { OrderSummary } from "@/generated/OrderSummary";
import { subjectName } from "../subjects";
import { orderStatus } from "./labels";

export const orderPath = (orderId: string) => `/orders/${orderId}`;

const patientName = (order: OrderSummary) =>
  subjectName({ familyName: order.patientFamilyName, givenNames: order.patientGivenNames });

const sampleBarcodes = (order: OrderSummary) => order.sampleBarcodes.join("; ");

/** Patient identity and source context remain visible before opening an order. */
export const ORDER_COLUMNS: readonly DataGridColumn<OrderSummary>[] = [
  {
    id: "patient",
    header: "Patient",
    cell: patientName,
    sortValue: patientName,
    identifier: true,
  },
  { id: "birthDate", header: "Birth date", cell: (order) => order.patientBirthDate, width: 120 },
  {
    id: "identifiers",
    header: "Patient identifiers",
    cell: (order) =>
      order.patientIdentifiers
        .map((i) => `${i.assigningAuthority} / ${i.identifierType}: ${i.value}`)
        .join("; ") || "None supplied",
  },
  {
    id: "placer",
    header: "Order number",
    cell: (order) => order.placerOrderId ?? "Not supplied",
    sortValue: (order) => order.placerOrderId ?? "",
    width: 130,
  },
  {
    id: "sampleBarcodes",
    header: "Sample barcode",
    cell: (order) => sampleBarcodes(order) || "None received",
    sortValue: sampleBarcodes,
    width: 150,
  },
  {
    id: "client",
    header: "Client",
    cell: (order) => order.clientName,
    sortValue: (order) => order.clientName,
  },
  {
    id: "items",
    header: "Tests",
    cell: (order) => order.itemCount,
    sortValue: (order) => order.itemCount,
    align: "end",
    width: 80,
  },
  {
    id: "received",
    header: "Received",
    cell: (order) => <Timestamp value={order.receivedAt} />,
    sortValue: (order) => order.receivedAt,
    width: 180,
  },
  {
    id: "status",
    header: "Status",
    cell: (order) => {
      const status = orderStatus(order.status);
      return <StatusBadge tone={status.tone}>{status.label}</StatusBadge>;
    },
    sortValue: (order) => order.status,
    width: 230,
  },
  {
    id: "holds",
    header: "Open holds",
    cell: (order) =>
      order.openHoldCount === 0 ? (
        0
      ) : (
        <StatusBadge tone="hold">{`${order.openHoldCount} open`}</StatusBadge>
      ),
    sortValue: (order) => order.openHoldCount,
    align: "end",
    width: 110,
  },
];
