import { useQuery } from "@tanstack/react-query";
import { useState } from "react";
import { FormProvider, useForm } from "react-hook-form";
import { Button } from "@/components/Button";
import { Form } from "@/components/Form";
import { RecordPickerInput } from "@/components/FormFields";
import { type RecordOption, RecordPicker } from "@/components/RecordPicker";
import { GallerySection } from "./GallerySection";
import styles from "./gallery.module.css";

/** Synthetic practitioners, two with the same name: the NPI-like code tells them apart. */
const PRACTITIONERS: RecordOption[] = Array.from({ length: 30 }, (_, index) => ({
  id: `prac-${index}`,
  label: index < 2 ? "Synthetic, Avery" : `Synthetic, Practitioner ${index + 1}`,
  description: `NPI 00000000${String(index).padStart(2, "0")}`,
}));

/** 10,000 synthetic practitioners, the NFR-33 bound, to check that the picker stays quick. */
const MANY_PRACTITIONERS: RecordOption[] = Array.from({ length: 10_000 }, (_, index) => ({
  id: `many-${index}`,
  label: `Synthetic, Practitioner ${String(index + 1).padStart(5, "0")}`,
  description: `NPI ${String(index + 1).padStart(10, "0")}`,
}));

const CLIENTS: RecordOption[] = [
  { id: "c1", label: "Synthetic Clinic North", description: "SYN-CL-01" },
  { id: "c2", label: "Synthetic Clinic South", description: "SYN-CL-02" },
];

const LOCATIONS = [
  { id: "l1", clientId: "c1", label: "North main office" },
  { id: "l2", clientId: "c1", label: "North annex" },
  { id: "l3", clientId: "c2", label: "South main office" },
];

/** A local stand-in for engine search: answers after 300ms, with no engine call. */
function useSyntheticSearch(text: string) {
  return useQuery({
    queryKey: ["gallery-practitioner-search", text],
    queryFn: () =>
      new Promise<RecordOption[]>((resolve) =>
        setTimeout(
          () => resolve(PRACTITIONERS.filter((each) => each.label.includes(text)).slice(0, 10)),
          300,
        ),
      ),
  });
}

/**
 * Choosing one record: from a list, from 10,000 records (virtualized, NFR-33), by engine
 * search, and a choice that depends on another.
 */
export function PickSection() {
  const [listed, setListed] = useState<string | null>(null);
  const [many, setMany] = useState<string | null>(null);
  const [multiple, setMultiple] = useState<string[]>([]);
  const [searched, setSearched] = useState<string | null>(null);
  const [text, setText] = useState("");
  const search = useSyntheticSearch(text);
  const form = useForm<{ clientId: string | null; locationId: string | null }>({
    defaultValues: { clientId: null, locationId: null },
  });
  const clientId = form.watch("clientId");
  return (
    <GallerySection title="Pick records">
      <div className={styles.stack}>
        <RecordPicker
          label="Practitioner (from a list)"
          name="listed"
          options={PRACTITIONERS}
          value={listed}
          onChange={setListed}
        />
        <RecordPicker
          label="Practitioner (from 10,000 records)"
          name="many"
          options={MANY_PRACTITIONERS}
          value={many}
          onChange={setMany}
        />
        <RecordPicker
          label="Practitioner (engine search)"
          name="searched"
          options={search.data ?? []}
          isLoading={search.isFetching}
          onInputChange={setText}
          value={searched}
          onChange={setSearched}
        />
        <RecordPicker
          label="Practitioners (multiple)"
          name="multiple"
          selectionMode="multiple"
          options={PRACTITIONERS}
          value={multiple}
          onChange={setMultiple}
        />
        <ul aria-label="Selected practitioners">
          {multiple.map((id) => (
            <li key={id}>
              {PRACTITIONERS.find((record) => record.id === id)?.label}
              <Button onPress={() => setMultiple(multiple.filter((value) => value !== id))}>
                Remove {PRACTITIONERS.find((record) => record.id === id)?.description}
              </Button>
            </li>
          ))}
        </ul>
        <FormProvider {...form}>
          <Form
            aria-label="Dependent choice"
            onSubmit={form.handleSubmit(() => undefined)}
            actions={<Button type="submit">Check fields</Button>}
          >
            <RecordPickerInput
              name="clientId"
              label="Client"
              options={CLIENTS}
              isRequired
              onChange={() => form.setValue("locationId", null)}
            />
            <RecordPickerInput
              name="locationId"
              label="Location"
              options={LOCATIONS.filter((location) => location.clientId === clientId)}
              description="Lists only the chosen client's locations, and clears when the client changes."
              isRequired
            />
          </Form>
        </FormProvider>
      </div>
    </GallerySection>
  );
}
