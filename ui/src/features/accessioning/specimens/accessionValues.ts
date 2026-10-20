import type { CollectionTimeInput } from "@/generated/CollectionTimeInput";
import type { CreateAccessionInput } from "@/generated/CreateAccessionInput";
import { checked, choiceOf, omittedIfBlank } from "@/lib/choices";
import { COLLECTION_TIME_KINDS, VOLUME_KINDS } from "./labels";

/** The accession form as typed; nothing about the collection is assumed. */
export interface AccessionFormValues {
  orderItemIds: string[];
  timeKind: string | null;
  collectedDate: string;
  collectedTime: string;
  utcOffset: string;
  timeZone: string;
  collector: string;
  specimens: {
    specimenTypeCode: string | null;
    containerCount: string;
    volume: string;
    volumeUnit: string;
    volumeKind: string | null;
  }[];
}

export const EMPTY_SPECIMEN: AccessionFormValues["specimens"][number] = {
  specimenTypeCode: null,
  containerCount: "",
  volume: "",
  volumeUnit: "",
  volumeKind: null,
};

export const EMPTY_ACCESSION: AccessionFormValues = {
  orderItemIds: [],
  timeKind: null,
  collectedDate: "",
  collectedTime: "",
  utcOffset: "",
  timeZone: "",
  collector: "",
  specimens: [EMPTY_SPECIMEN],
};

/** The `create_accession` input from validated form values. */
export function toCreateAccessionInput(
  orderId: string,
  values: AccessionFormValues,
): CreateAccessionInput {
  return {
    orderId,
    orderItemIds: values.orderItemIds,
    collection: {
      time: collectionTime(values),
      collector: omittedIfBlank(values.collector),
      observations: [],
    },
    specimens: values.specimens.map((specimen) => ({
      specimenTypeCode: checked(specimen.specimenTypeCode),
      containerCount: Number.parseInt(specimen.containerCount, 10),
      // A clinical quantity: sent exactly as typed, never as a number.
      volume: omittedIfBlank(specimen.volume),
      volumeUnit: omittedIfBlank(specimen.volumeUnit),
      volumeKind: checked(choiceOf(VOLUME_KINDS, specimen.volumeKind)),
    })),
  };
}

export function collectionTime(values: AccessionFormValues): CollectionTimeInput {
  const kind = checked(choiceOf(COLLECTION_TIME_KINDS, values.timeKind));
  if (kind === "unknown") return { kind };
  if (kind === "date") return { kind, date: values.collectedDate.trim() };
  const local = `${values.collectedDate.trim()}T${values.collectedTime.trim()}`;
  if (kind === "zoned") return { kind, local, timeZone: values.timeZone.trim() };
  return {
    kind,
    local,
    utcOffsetMinutes: offsetMinutes(values.utcOffset.trim()),
  };
}

/** `-05:00` as -300; the field's format check has already accepted the text. */
function offsetMinutes(text: string): number {
  const sign = text.startsWith("-") ? -1 : 1;
  const [hours = "0", minutes = "0"] = text.slice(1).split(":");
  return sign * (Number.parseInt(hours, 10) * 60 + Number.parseInt(minutes, 10));
}
