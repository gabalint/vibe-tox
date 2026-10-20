import { type KeyboardEvent, useId, useLayoutEffect, useRef, useState } from "react";
import { useLocation } from "react-router";
import { articleForPath, articles } from "./articles";
import { HelpArticleView } from "./HelpArticleView";
import { HelpHeader } from "./HelpHeader";
import { type HelpLayout, HelpSplitter } from "./HelpResize";
import { HelpTopics } from "./HelpTopics";
import styles from "./PageHelp.module.css";

interface PageHelpProps {
  isOpen: boolean;
  layout: HelpLayout;
  controls: string;
  onClose: () => void;
}

/** Navigation resets the topic without remounting the page or moving focus out of it. */
export function PageHelp({ isOpen, onClose, layout, controls }: PageHelpProps) {
  const { pathname } = useLocation();
  const currentArticle = articleForPath(pathname);
  const [shownPath, setShownPath] = useState(pathname);
  const [selectedId, setSelectedId] = useState(currentArticle?.id);
  const [browse, setBrowse] = useState(false);
  const [selection, setSelection] = useState(0);
  if (shownPath !== pathname) {
    setShownPath(pathname);
    setSelectedId(currentArticle?.id);
    setBrowse(false);
  }
  const article = articles.find((item) => item.id === selectedId);
  const heading = useRef<HTMLHeadingElement>(null);
  const body = useRef<HTMLElement>(null);
  const titleId = useId();

  // Opening help moves focus into it; closing restores focus through the shell's trigger.
  useLayoutEffect(() => {
    if (isOpen) heading.current?.focus();
  }, [isOpen]);

  // Each navigation starts the newly selected topic at its beginning.
  // biome-ignore lint/correctness/useExhaustiveDependencies: pathname marks a navigation, not the scroll value.
  useLayoutEffect(() => {
    if (body.current) body.current.scrollTop = 0;
  }, [pathname]);

  function selectArticle(id: string | undefined) {
    setSelection((last) => last + 1);
    setSelectedId(id);
    setBrowse(false);
    if (body.current) body.current.scrollTop = 0;
    heading.current?.focus();
  }

  function closeOnEscape(event: KeyboardEvent<HTMLElement>) {
    if (event.key !== "Escape" || event.defaultPrevented) return;
    event.preventDefault();
    event.stopPropagation();
    onClose();
  }

  function browseTopics() {
    setBrowse(true);
    if (body.current) body.current.scrollTop = 0;
    heading.current?.focus();
  }

  return (
    // biome-ignore lint/a11y/noNoninteractiveElementInteractions: Escape closes help only while focus is inside it.
    <aside aria-labelledby={titleId} className={styles.panel} onKeyDown={closeOnEscape}>
      <HelpSplitter layout={layout} controls={controls} />
      <HelpHeader
        layout={layout}
        titleId={titleId}
        headingRef={heading}
        isBrowsing={browse}
        onCurrentPage={() => selectArticle(currentArticle?.id)}
        onBrowse={browseTopics}
        onClose={onClose}
      />
      {/* biome-ignore lint/a11y/noNoninteractiveTabindex: The scrollable help region must be keyboard-scrollable in all desktop webviews. */}
      <section ref={body} className={styles.body} aria-label="Help content" tabIndex={0}>
        {browse ? (
          <HelpTopics onSelect={selectArticle} />
        ) : article ? (
          <HelpArticleView
            key={`${article.id}:${pathname}:${selection}`}
            article={article}
            pathname={pathname}
          />
        ) : (
          <p>Help for this page is not available. Browse all topics for related guidance.</p>
        )}
      </section>
    </aside>
  );
}
