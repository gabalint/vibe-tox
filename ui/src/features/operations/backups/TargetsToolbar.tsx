import type { ReactNode } from "react";
import { Button } from "@/components/Button";
import { Checkbox } from "@/components/Checkbox";
import { Toolbar } from "@/components/Toolbar";

interface TargetsToolbarProps {
  add: ReactNode;
  /** "2 of 3 targets" once the targets are loaded. */
  count: string | undefined;
  showRetired: boolean;
  onShowRetired: (show: boolean) => void;
}

/** Add target, "Show retired" with its "Clear filters", and the count. */
export function TargetsToolbar({ add, count, showRetired, onShowRetired }: TargetsToolbarProps) {
  return (
    <Toolbar variant="collection" count={count}>
      <Checkbox isSelected={showRetired} onChange={onShowRetired}>
        Show retired
      </Checkbox>
      {showRetired && (
        <Button variant="quiet" onPress={() => onShowRetired(false)}>
          Clear filters
        </Button>
      )}
      {add}
    </Toolbar>
  );
}
