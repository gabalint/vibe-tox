import { X } from "lucide-react";
import type { Ref } from "react";
import { Button } from "@/components/Button";
import { type HelpLayout, HelpSizeMenu } from "./HelpResize";
import styles from "./PageHelp.module.css";

interface HelpHeaderProps {
  titleId: string;
  layout: HelpLayout;
  headingRef: Ref<HTMLHeadingElement>;
  isBrowsing: boolean;
  onCurrentPage: () => void;
  onBrowse: () => void;
  onClose: () => void;
}

export function HelpHeader(props: HelpHeaderProps) {
  return (
    <>
      <header className={styles.header}>
        <h2 id={props.titleId} ref={props.headingRef} tabIndex={-1}>
          Page help
        </h2>
        <HelpSizeMenu layout={props.layout} />
        <Button variant="quiet" aria-label="Close page help" onPress={props.onClose}>
          <X aria-hidden="true" />
        </Button>
      </header>
      <div className={styles.tools}>
        <Button variant="quiet" onPress={props.onCurrentPage}>
          Current page
        </Button>
        <Button variant="quiet" isDisabled={props.isBrowsing} onPress={props.onBrowse}>
          Browse all topics
        </Button>
      </div>
    </>
  );
}
