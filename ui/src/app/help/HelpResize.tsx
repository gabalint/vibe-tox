import { BookOpen, Ellipsis } from "lucide-react";
import type { Ref } from "react";
import { Button, MenuTrigger } from "react-aria-components";
import { Button as AppButton } from "@/components/Button";
import { ActionMenu } from "@/components/RowActions/ActionMenu";
import styles from "./PageHelp.module.css";
import type { useHelpLayout } from "./useHelpLayout";

export type HelpLayout = ReturnType<typeof useHelpLayout>;

export function HelpSplitter({ layout, controls }: { layout: HelpLayout; controls: string }) {
  return (
    // biome-ignore lint/a11y/useSemanticElements: This focusable separator resizes a pane; it is not a static hr.
    <div
      role="separator"
      tabIndex={0}
      aria-label={`Page help ${layout.stacked ? "height" : "width"}`}
      aria-description={`Use ${layout.stacked ? "Up and Down" : "Left and Right"} arrows to resize; Home for minimum, End for maximum.`}
      aria-orientation={layout.stacked ? "horizontal" : "vertical"}
      aria-controls={controls}
      aria-valuemin={layout.min}
      aria-valuemax={layout.max}
      aria-valuenow={layout.size}
      aria-valuetext={`${layout.size} pixels`}
      className={styles.resize}
      data-stacked={layout.stacked}
      data-resizing={layout.dragging}
      {...layout.separatorProps}
    />
  );
}

export function HelpSizeMenu({ layout }: { layout: HelpLayout }) {
  return (
    <MenuTrigger>
      <Button aria-label="Page help size options" className={styles.sizeButton}>
        <Ellipsis aria-hidden="true" />
      </Button>
      <ActionMenu
        items={[
          {
            id: "smaller",
            label: layout.stacked ? "Shorter panel" : "Narrower panel",
            onAction: layout.smaller,
          },
          {
            id: "larger",
            label: layout.stacked ? "Taller panel" : "Wider panel",
            onAction: layout.larger,
          },
          { id: "reset", label: "Reset panel size", onAction: layout.reset },
        ]}
      />
    </MenuTrigger>
  );
}

export function helpGridStyle(layout: HelpLayout, open: boolean) {
  return {
    gridTemplateColumns:
      open && !layout.stacked ? `auto minmax(0, 1fr) ${layout.size}px` : undefined,
    gridTemplateRows:
      open && layout.stacked ? `auto auto minmax(0, 1fr) ${layout.size}px` : undefined,
  };
}

export function PageHelpButton({
  open,
  controls,
  buttonRef,
  onPress,
}: {
  open: boolean;
  controls: string;
  buttonRef: Ref<HTMLButtonElement>;
  onPress: () => void;
}) {
  return (
    <AppButton
      ref={buttonRef}
      variant="quiet"
      aria-expanded={open}
      aria-controls={controls}
      onPress={onPress}
    >
      <BookOpen aria-hidden="true" />
      Page help
    </AppButton>
  );
}
