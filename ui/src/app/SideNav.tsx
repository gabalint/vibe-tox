import { useEffect, useState } from "react";
import { useLocation } from "react-router";
import { LIST_WORK_QUEUES, type WorkQueueSummary } from "@/features/workqueues/contract";
import { useCommandQuery } from "@/lib/commands";
import { NavGroup } from "./NavGroup";
import { NavItem, type QueueCount } from "./NavItem";
import { ALL_QUEUES, type NavSection, navigation, type Screen } from "./navigation";
import styles from "./SideNav.module.css";

/** How often the queue counts are asked for again; the count is a cheap, unaudited read. */
const COUNT_POLL_MS = 60_000;

/** The local UI preference: which work areas the person opened or closed. No clinical data. */
const OPEN_SECTIONS_KEY = "vibe-tox.navigation.open-sections";

type OpenSections = Record<string, boolean>;

/**
 * The side navigation: the work areas the session's capabilities open, each collapsible, with
 * the engine's queue counts beside the entries. This is presentation only: the engine refuses
 * every command the person lacks the capability for. It scrolls on its own, so the top bar
 * stays in place however many entries an administrator sees.
 */
export function SideNav({
  capabilities,
  collapsed,
}: {
  capabilities: readonly string[];
  collapsed: boolean;
}) {
  const { pathname } = useLocation();
  const sections = visibleSections(capabilities);
  const current = currentScreen(sections, pathname);
  const active = sections.find(
    (section) => current !== undefined && section.screens.includes(current),
  );
  const [open, toggle] = useOpenSections(active);
  const count = useQueueCounts();
  const items = (screens: readonly Screen[], onNavigate?: () => void) =>
    screens.map((screen) => (
      <NavItem
        key={screen.path}
        screen={screen}
        current={screen === current ? currentKind(screen, pathname) : undefined}
        count={screen.queue === undefined ? null : count(screen.queue)}
        compact={collapsed && screen.queue === ALL_QUEUES}
        onNavigate={onNavigate}
      />
    ));
  return (
    <nav
      id="main-navigation"
      aria-label="Main"
      className={styles.nav}
      data-compact={collapsed || undefined}
    >
      <ul className={styles.list}>
        {sections.map((section) =>
          section.heading === undefined ? (
            items(section.screens)
          ) : (
            <NavGroup
              key={section.heading}
              heading={section.heading}
              icon={section.icon}
              compact={collapsed}
              active={section === active}
              count={count(
                section.screens.flatMap((screen) => (screen.queue ? [screen.queue] : [])),
              )}
              isOpen={open(section)}
              onToggle={() => toggle(section)}
            >
              {(onNavigate) => items(section.screens, onNavigate)}
            </NavGroup>
          ),
        )}
      </ul>
    </nav>
  );
}

function visibleSections(capabilities: readonly string[]): NavSection[] {
  return navigation
    .map((section) => ({
      ...section,
      screens: section.screens.filter(
        (screen) =>
          screen.capabilities === undefined ||
          screen.capabilities.some((capability) => capabilities.includes(capability)),
      ),
    }))
    .filter((section) => section.screens.length > 0);
}

/**
 * The one entry marked current: the screen itself, or the screen a detail screen sits under
 * (the longest matching path), so `/reports/release` never also marks `/reports`.
 */
function currentScreen(sections: readonly NavSection[], pathname: string): Screen | undefined {
  return sections
    .flatMap((section) => section.screens)
    .filter((screen) => pathname === screen.path || pathname.startsWith(`${screen.path}/`))
    .sort((a, b) => b.path.length - a.path.length)[0];
}

function currentKind(screen: Screen, pathname: string): "page" | "true" {
  return pathname === screen.path ? "page" : "true";
}

/**
 * Which work areas are open: the person's choice, remembered on this computer, else closed, so
 * the first view is a short list. Going to a screen opens its area.
 */
function useOpenSections(
  active: NavSection | undefined,
): [(section: NavSection) => boolean, (section: NavSection) => void] {
  const [prefs, setPrefs] = useState(readOpenSections);
  // Sections are rebuilt each render, so the active one is tracked by its heading.
  const [shownFor, setShownFor] = useState<string | undefined>();
  const isOpen = (section: NavSection) => prefs[section.heading ?? ""] ?? false;
  if (active?.heading !== shownFor) {
    setShownFor(active?.heading);
    if (active?.heading !== undefined && !isOpen(active)) {
      setPrefs({ ...prefs, [active.heading]: true });
    }
  }
  useEffect(() => {
    try {
      localStorage.setItem(OPEN_SECTIONS_KEY, JSON.stringify(prefs));
    } catch {
      // Navigation still works when local presentation preferences cannot be stored.
    }
  }, [prefs]);
  const toggle = (section: NavSection) =>
    setPrefs((last) => ({ ...last, [section.heading ?? ""]: !isOpen(section) }));
  return [isOpen, toggle];
}

function readOpenSections(): OpenSections {
  try {
    const stored: unknown = JSON.parse(localStorage.getItem(OPEN_SECTIONS_KEY) ?? "{}");
    if (typeof stored !== "object" || stored === null) return {};
    return Object.fromEntries(
      Object.entries(stored).filter(([, value]) => typeof value === "boolean"),
    );
  } catch {
    return {};
  }
}

/** Counts only from `list_work_queues`; none while it is loading or failed, never a guess. */
function useQueueCounts(): (queue: string | string[]) => QueueCount | null {
  const queues = useCommandQuery<WorkQueueSummary[]>(LIST_WORK_QUEUES, undefined, {
    refetchInterval: COUNT_POLL_MS,
  });
  return (queue) => {
    if (queues.isError) return null;
    const counted = (queues.data ?? []).filter(
      (summary) =>
        queue === ALL_QUEUES ||
        (Array.isArray(queue) ? queue.includes(summary.queue) : summary.queue === queue),
    );
    const total = counted.reduce((sum, summary) => sum + (summary.count ?? 0), 0);
    if (counted.some((summary) => summary.count === null)) {
      return {
        shown: total > 0 ? `${total}+` : "?",
        spoken: `${total} known waiting; some queues not counted`,
      };
    }
    if (total === 0) return null;
    return counted.some((summary) => summary.countCapped)
      ? { shown: `${total}+`, spoken: `${total} or more waiting` }
      : { shown: String(total), spoken: `${total} waiting` };
  };
}
