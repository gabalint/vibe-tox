import { Button } from "@/components/Button";
import type { ActionItem } from "./ActionMenu";
import { MoreActions } from "./RowActions";

/** Actions shown as buttons; the rest go under "More" (UI standards, record actions). */
const BUTTONS = 3;

/**
 * A record view's actions from the same items as its grid row's menu, so the two never
 * differ: the first as the primary button, up to two more buttons (red text when destructive),
 * then "More". Renders nothing when `items` is empty.
 */
export function RecordActions({ items }: { items: readonly ActionItem[] }) {
  return (
    <>
      {items.slice(0, BUTTONS).map((item, index) => (
        <Button
          key={item.id}
          variant={item.variant === "danger" ? "danger" : index === 0 ? "primary" : "secondary"}
          onPress={item.onAction}
        >
          {item.label}
        </Button>
      ))}
      <MoreActions items={items.slice(BUTTONS)} />
    </>
  );
}
