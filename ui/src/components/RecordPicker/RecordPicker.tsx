import { Check, ChevronDown, X } from "lucide-react";
import { type ComponentProps, type KeyboardEvent, type Ref, useContext, useState } from "react";
import {
  Button,
  ComboBox,
  ComboBoxStateContext,
  Input,
  ListBox,
  ListBoxItem,
  ListLayout,
  Popover,
  Text,
  Virtualizer,
} from "react-aria-components";
import { useTypingPause } from "@/components/FindField/useTypingPause";
import { FieldHelp } from "@/components/Form/FieldHelp";
import { FieldLabel } from "@/components/Form/FieldLabel";
import formStyles from "@/components/Form/Form.module.css";
import selectStyles from "@/components/Select/Select.module.css";
import styles from "./RecordPicker.module.css";

/** One existing record to choose. */
export interface RecordOption {
  id: string;
  label: string;
  /** What tells near-duplicates apart, such as a code or an NPI; shown and searched. */
  description?: string;
}

interface RecordPickerBaseProps {
  label: string;
  /** Compact collection filter; form fields keep their stacked label. */
  layout?: "inline";
  /** Reads the label to screen readers only, as `Select`'s does. */
  isLabelHidden?: boolean;
  /** What to type, such as the values searched. */
  placeholder?: string;
  /** Also the key for engine field errors shown by `Form`. */
  name: string;
  /** The records to choose from; with engine search, the answer for the typed text. */
  options: readonly RecordOption[];
  /**
   * Engine search: called with the typed text after a short pause; pass the answer as
   * `options`. Without it, the picker filters `options` itself.
   */
  onInputChange?: (text: string) => void;
  /** Engine search: the answer for the typed text is still loading. */
  isLoading?: boolean;
  /** A single choice shows a clear button while it holds a record, as a filter needs. */
  isClearable?: boolean;
  /** `stacked` puts each option's description under its label, for long descriptions. */
  optionLayout?: "stacked";
  isRequired?: boolean;
  isDisabled?: boolean;
  description?: string;
  errorMessage?: string | undefined;
  /** The text input, so a form can focus the field when it is invalid. */
  inputRef?: Ref<HTMLInputElement>;
}

type RecordPickerProps = RecordPickerBaseProps &
  (
    | { selectionMode?: "single"; value: string | null; onChange: (value: string | null) => void }
    | {
        /** The caller keeps selected records visible outside the results, with removal controls. */
        selectionMode: "multiple";
        value: readonly string[];
        onChange: (value: string[]) => void;
        onInputChange?: never;
      }
  );

/** The option as the field shows it once chosen, and as typing searches it. */
const optionText = ({ label, description }: RecordOption) =>
  description ? `${label} (${description})` : label;

const noSearch = () => undefined;

/**
 * Chooses existing records by typing part of their name or description; free text is never
 * kept. It starts empty: nothing the requisition states is preselected (REQ-ORD-005).
 *
 * A choice that depends on another, such as a location after a client, gets only the
 * matching records as `options`, and the caller sets it to `null` when the parent changes
 * (`RecordPickerInput`'s `onChange` on the parent). With engine search, `options` must
 * include the chosen record when the field starts with one.
 */
export function RecordPicker(props: RecordPickerProps) {
  const {
    label,
    layout,
    isLabelHidden,
    placeholder,
    options,
    onInputChange,
    isLoading = false,
    isClearable = false,
    optionLayout,
    description,
    errorMessage,
    isRequired,
    inputRef,
  } = props;
  // With engine search, the chosen record stays listed when a later answer leaves it out, so
  // leaving the field restores its text instead of clearing the choice.
  const [chosen, setChosen] = useState<RecordOption | null>(null);
  const pause = useTypingPause(onInputChange ?? noSearch);
  const keepChosen =
    chosen !== null && chosen.id === props.value && !options.some(({ id }) => id === chosen.id);
  const items = keepChosen ? [chosen, ...options] : options;
  const search = (text: string) => {
    // Choosing a record writes its text into the field; that is not a new search.
    if (chosen === null || text !== optionText(chosen)) pause.later(text);
  };
  return (
    <ComboBox<RecordOption, "single" | "multiple">
      name={props.name}
      isDisabled={props.isDisabled}
      selectionMode={props.selectionMode ?? "single"}
      value={props.value}
      data-layout={layout}
      {...(onInputChange ? { items, onInputChange: search } : { defaultItems: options })}
      validationBehavior="aria"
      allowsEmptyCollection
      isRequired={isRequired}
      isInvalid={errorMessage === undefined ? undefined : true}
      onChange={(key) => {
        if (props.selectionMode === "multiple") {
          if (Array.isArray(key)) props.onChange(key.map(String));
          return;
        }
        const id = key === null ? null : String(key);
        setChosen(items.find((option) => option.id === id) ?? null);
        props.onChange(id);
      }}
      className={formStyles.field}
    >
      <FieldLabel isRequired={isRequired} isHidden={isLabelHidden}>
        {label}
      </FieldLabel>
      <div className={styles.control} data-clearable={isClearable || undefined}>
        <PickerInput
          ref={inputRef}
          placeholder={placeholder}
          autoCorrect="off"
          autoCapitalize="off"
          spellCheck={false}
          onKeyDownCapture={props.selectionMode === "multiple" ? selectSearchText : undefined}
          onKeyDown={props.selectionMode === "multiple" ? keepSearchEnter : undefined}
          className={`${formStyles.control} ${styles.input}`}
        />
        {isClearable && props.selectionMode !== "multiple" && props.value !== null && (
          <Button
            // Out of the ComboBox's button slot, which would make it a second list opener.
            slot={null}
            aria-label={`Clear ${label}`}
            className={styles.clear}
            onPress={() => {
              setChosen(null);
              props.onChange(null);
            }}
          >
            <X aria-hidden="true" />
          </Button>
        )}
        <Button className={styles.opener}>
          <ChevronDown aria-hidden="true" className={selectStyles.chevron} />
        </Button>
      </div>
      <FieldHelp description={description} errorMessage={errorMessage} />
      <OptionList isLoading={isLoading} layout={optionLayout} />
    </ComboBox>
  );
}

/**
 * The text input; a click anywhere in it opens the full list, as the arrow and a `Select`
 * do, and typing still narrows it. Keyboard focus alone does not open it.
 */
function PickerInput(props: ComponentProps<typeof Input>) {
  const state = useContext(ComboBoxStateContext);
  return (
    <Input
      {...props}
      onClick={() => {
        if (state && !state.isOpen) state.open(null, "manual");
      }}
    />
  );
}

/** Preserve text editing: React Aria's multiple list selection otherwise consumes Ctrl/Cmd+A. */
function selectSearchText(event: KeyboardEvent<HTMLInputElement>) {
  if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "a") {
    event.preventDefault();
    event.stopPropagation();
    event.currentTarget.select();
  }
}

/** Choosing search results must not submit the surrounding form, including with a closed popup. */
function keepSearchEnter(event: KeyboardEvent<HTMLInputElement>) {
  if (event.key === "Enter" && !event.nativeEvent.isComposing) event.preventDefault();
}

/**
 * Each option's estimated height in pixels, `--control-height`; options whose text wraps are
 * measured.
 */
const OPTION_HEIGHT = 32;
const LAYOUT_OPTIONS = { estimatedRowHeight: OPTION_HEIGHT };

/**
 * The options, each with the text that tells it apart, or why there are none. Virtualized,
 * so only the options in view are in the page and a list of 10,000 records opens and
 * narrows as quickly as a short one (NFR-33).
 */
function OptionList({ isLoading, layout }: { isLoading: boolean; layout?: "stacked" }) {
  return (
    <Popover className={`${selectStyles.popover} ${styles.popover}`}>
      <Virtualizer layout={ListLayout} layoutOptions={LAYOUT_OPTIONS}>
        <ListBox
          className={`${selectStyles.listBox} ${styles.listBox}`}
          renderEmptyState={() => (
            <p className={styles.empty}>{isLoading ? "Searching…" : "No matches"}</p>
          )}
        >
          {(option: RecordOption) => (
            <ListBoxItem
              id={option.id}
              textValue={optionText(option)}
              className={`${selectStyles.option} ${styles.option}`}
            >
              <Check aria-hidden="true" className={styles.selectedIcon} />
              <span className={styles.texts} data-layout={layout}>
                <Text slot="label">{option.label}</Text>
                {option.description && (
                  <Text slot="description" className={styles.description}>
                    {option.description}
                  </Text>
                )}
              </span>
            </ListBoxItem>
          )}
        </ListBox>
      </Virtualizer>
    </Popover>
  );
}
