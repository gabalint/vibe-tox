import { ChevronDown, Ellipsis } from "lucide-react";
import { Button as AriaButton, MenuTrigger } from "react-aria-components";
import { Button } from "@/components/Button";
import { type ActionItem, ActionMenu } from "./ActionMenu";
import styles from "./RowActions.module.css";

interface RowActionsProps {
  /** The row's identifier; the button is named "Actions for <identifier>". */
  identifier: string;
  /** The row's actions, from the record's `allowedActions`. */
  items: readonly ActionItem[];
  /** Controlled open state, for a grid that also opens the menu by right-click or Shift+F10. */
  isOpen?: boolean;
  /** Called when the menu opens or closes. */
  onOpenChange?: (isOpen: boolean) => void;
}

/**
 * The "⋯" button of one grid row and its menu: the same actions as the record view, as a
 * shortcut. The button is out of the Tab order, since the grid is one Tab stop; `DataGrid`
 * renders it through its `rowActions` option.
 */
export function RowActions({ identifier, items, isOpen, onOpenChange }: RowActionsProps) {
  return (
    <MenuTrigger isOpen={isOpen} onOpenChange={onOpenChange}>
      <AriaButton
        aria-label={`Actions for ${identifier}`}
        excludeFromTabOrder
        className={styles.trigger}
      >
        <Ellipsis aria-hidden="true" />
      </AriaButton>
      <ActionMenu items={items} />
    </MenuTrigger>
  );
}

/**
 * The "More" menu of a record view (side panel or page header), for the actions beyond the
 * primary button and up to two more. Renders nothing when there are no items.
 */
export function MoreActions({ items }: { items: readonly ActionItem[] }) {
  if (items.length === 0) return null;
  return (
    <MenuTrigger>
      <Button>
        More
        <ChevronDown aria-hidden="true" />
      </Button>
      <ActionMenu items={items} />
    </MenuTrigger>
  );
}
