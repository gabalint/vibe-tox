import { screen, waitFor, within } from "@testing-library/react";
import { useState } from "react";
import { describe, expect, it, vi } from "vitest";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { pickRecord } from "@/lib/test/picker";
import { renderWithProviders } from "@/lib/test/render";
import { type RecordOption, RecordPicker } from "./RecordPicker";

const PRACTITIONERS: RecordOption[] = [
  { id: "p1", label: "Synthetic, Avery", description: "NPI 0000000011" },
  { id: "p2", label: "Synthetic, Avery", description: "NPI 0000000029" },
  { id: "p3", label: "Example, Blake", description: "NPI 0000000037" },
];

function Static({
  onChange,
  options = PRACTITIONERS,
}: {
  onChange: (value: string | null) => void;
  options?: readonly RecordOption[];
}) {
  const [value, setValue] = useState<string | null>(null);
  return (
    <RecordPicker
      label="Practitioner"
      name="practitionerId"
      options={options}
      value={value}
      onChange={(next) => {
        setValue(next);
        onChange(next);
      }}
    />
  );
}

const field = () => screen.getByRole("combobox", { name: "Practitioner" });
const optionTexts = () =>
  within(screen.getByRole("listbox"))
    .queryAllByRole("option")
    .map((option) => option.textContent);

describe("RecordPicker with a static list", () => {
  it("starts empty and filters by name and by the text that tells near-duplicates apart", async () => {
    const onChange = vi.fn();
    const { user, container } = renderWithProviders(<Static onChange={onChange} />);
    expect(field()).toHaveProperty("value", "");
    await expectNoA11yViolations(container);
    await user.type(field(), "avery");
    expect(optionTexts()).toEqual([
      "Synthetic, AveryNPI 0000000011",
      "Synthetic, AveryNPI 0000000029",
    ]);
    await user.clear(field());
    await user.type(field(), "0029");
    expect(optionTexts()).toEqual(["Synthetic, AveryNPI 0000000029"]);
    await user.keyboard("{ArrowDown}{Enter}");
    expect(onChange).toHaveBeenLastCalledWith("p2");
    // The chosen record shows with its description, so near-duplicates stay apart.
    expect(field()).toHaveProperty("value", "Synthetic, Avery (NPI 0000000029)");
  });

  it("says when nothing matches and keeps no free text", async () => {
    const onChange = vi.fn();
    const { user } = renderWithProviders(<Static onChange={onChange} />);
    await user.type(field(), "nobody");
    expect(screen.getByRole("listbox").textContent).toBe("No matches");
    await user.tab();
    expect(field()).toHaveProperty("value", "");
    expect(onChange).not.toHaveBeenCalled();
  });
});

/** 10,000 synthetic practitioners, the NFR-33 bound for a directory. */
const MANY: RecordOption[] = Array.from({ length: 10_000 }, (_, index) => ({
  id: `p${index}`,
  label: `Synthetic, Practitioner ${String(index).padStart(5, "0")}`,
  description: `NPI ${String(index).padStart(10, "0")}`,
}));

/**
 * jsdom has no layout: gives the option list a 320px view and each option a 32px height, so
 * the virtualizer has sizes to work with. Restored after each test (`restoreMocks`).
 */
function mockListSize() {
  vi.spyOn(HTMLElement.prototype, "clientWidth", "get").mockReturnValue(400);
  vi.spyOn(HTMLElement.prototype, "clientHeight", "get").mockReturnValue(320);
  vi.spyOn(Element.prototype, "scrollHeight", "get").mockReturnValue(32);
}

describe("RecordPicker with 10,000 records (NFR-33)", () => {
  it("renders only the options in view when opened and while narrowing", async () => {
    mockListSize();
    const onChange = vi.fn();
    const { user } = renderWithProviders(<Static onChange={onChange} options={MANY} />);
    await user.click(screen.getByRole("button"));
    expect(optionTexts().length).toBeGreaterThan(0);
    expect(optionTexts().length).toBeLessThan(50);
    await user.type(field(), "Practitioner 09");
    expect(optionTexts().length).toBeLessThan(50);
    expect(optionTexts()[0]).toBe("Synthetic, Practitioner 09000NPI 0000009000");
  });

  it("is chosen by keyboard from the full list", async () => {
    mockListSize();
    const onChange = vi.fn();
    const { user } = renderWithProviders(<Static onChange={onChange} options={MANY} />);
    await pickRecord(user, "Practitioner", "0000009999");
    expect(onChange).toHaveBeenLastCalledWith("p9999");
  });
});

function Searching({ onSearch }: { onSearch: (text: string) => void }) {
  const [value, setValue] = useState<string | null>(null);
  const [text, setText] = useState("");
  // A synthetic engine answer: the records whose name contains the text.
  const options = PRACTITIONERS.filter((option) =>
    option.label.toLowerCase().includes(text.toLowerCase()),
  );
  return (
    <RecordPicker
      label="Practitioner"
      name="practitionerId"
      options={text === "loading" ? [] : options}
      isLoading={text === "loading"}
      onInputChange={(next) => {
        setText(next);
        onSearch(next);
      }}
      value={value}
      onChange={setValue}
    />
  );
}

describe("RecordPicker with engine search", () => {
  it("searches after a pause in typing, not again on choosing, and keeps the choice", async () => {
    const onSearch = vi.fn();
    const { user } = renderWithProviders(<Searching onSearch={onSearch} />);
    await user.type(field(), "example");
    await waitFor(() => expect(onSearch).toHaveBeenCalledWith("example"));
    expect(onSearch).toHaveBeenCalledOnce();
    await user.keyboard("{ArrowDown}{Enter}");
    expect(field()).toHaveProperty("value", "Example, Blake (NPI 0000000037)");

    // A later answer that leaves the chosen record out does not clear the choice.
    await user.type(field(), "x");
    await waitFor(() => expect(onSearch).toHaveBeenCalledTimes(2));
    await user.tab();
    expect(field()).toHaveProperty("value", "Example, Blake (NPI 0000000037)");
  });

  it("shows that the answer is loading", async () => {
    const { user } = renderWithProviders(<Searching onSearch={() => undefined} />);
    await user.type(field(), "loading");
    await waitFor(() => expect(screen.getByRole("listbox").textContent).toBe("Searching…"));
  });
});

function Multiple({ onChange }: { onChange: (value: string[]) => void }) {
  const [value, setValue] = useState<string[]>([]);
  const [filtered, setFiltered] = useState(false);
  return (
    <>
      <RecordPicker
        label="Practitioners"
        name="practitioners"
        selectionMode="multiple"
        options={filtered ? PRACTITIONERS.slice(2) : PRACTITIONERS}
        value={value}
        onChange={(next) => {
          setValue(next);
          onChange(next);
        }}
      />
      <button type="button" onClick={() => setFiltered(!filtered)}>
        Filter options
      </button>
    </>
  );
}

describe("RecordPicker with multiple selection", () => {
  it("bounds the rendered options for a large multiple-choice list", async () => {
    mockListSize();
    const { user } = renderWithProviders(
      <RecordPicker
        label="Practitioners"
        name="practitioners"
        selectionMode="multiple"
        options={MANY}
        value={[]}
        onChange={() => undefined}
      />,
    );
    await user.click(screen.getByRole("button"));
    expect(optionTexts().length).toBeGreaterThan(0);
    expect(optionTexts().length).toBeLessThan(50);
  });

  it("keeps Enter in search from submitting and lets select-all replace search text", async () => {
    const submit = vi.fn();
    const onChange = vi.fn();
    const { user } = renderWithProviders(
      <form
        onSubmit={(event) => {
          event.preventDefault();
          submit();
        }}
      >
        <Multiple onChange={onChange} />
        <button type="submit">Save</button>
      </form>,
    );
    const input = screen.getByRole("combobox", { name: "Practitioners" });
    await user.click(input);
    await user.keyboard("{Enter}");
    expect(submit).not.toHaveBeenCalled();
    await user.type(input, "avery");
    await user.keyboard("{Control>}a{/Control}example");
    expect(input).toHaveProperty("value", "example");
    expect(onChange).not.toHaveBeenCalled();
  });

  it("keeps focus for consecutive keyboard selection and preserves choices across filtering", async () => {
    const onChange = vi.fn();
    const { user, container } = renderWithProviders(<Multiple onChange={onChange} />);
    const input = screen.getByRole("combobox", { name: "Practitioners" });
    await user.type(input, "0029");
    await user.keyboard("{ArrowDown}{Enter}");
    expect(onChange).toHaveBeenLastCalledWith(["p2"]);
    expect(document.activeElement).toBe(input);
    expect(input).toHaveProperty("value", "");
    await user.keyboard("{Escape}");
    await user.click(screen.getByRole("button", { name: "Filter options" }));
    await user.type(input, "0037");
    await user.keyboard("{ArrowDown}{Enter}");
    expect(onChange).toHaveBeenLastCalledWith(["p2", "p3"]);
    await user.keyboard("{Escape}");
    await expectNoA11yViolations(container);
    await user.type(input, "nobody");
    expect(screen.getByRole("listbox").textContent).toBe("No matches");
    await user.keyboard("{Enter}");
    expect(onChange).toHaveBeenCalledTimes(2);
  });
});
