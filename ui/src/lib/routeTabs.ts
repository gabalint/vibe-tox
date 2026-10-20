import { useParams } from "react-router";

/** One tab of a tabbed screen: its ID is its path segment. */
interface RouteTab<Id extends string> {
  id: Id;
  label: string;
}

/**
 * The `Tabs` of a screen whose tabs are routes `basePath/<id>` (the route's optional `:tab`
 * segment), from one list: each tab's `href`, and the selected tab. The base path (no
 * segment) and a segment that is no tab select the first tab.
 *
 * ```tsx
 * const tabs = useRouteTabs("/operations/backups", BACKUP_TABS);
 * <Tabs label="Backups" tabs={tabs.tabs} selectedId={tabs.selectedId}>…</Tabs>
 * ```
 */
export function useRouteTabs<const Id extends string>(
  basePath: string,
  tabs: readonly [RouteTab<Id>, ...RouteTab<Id>[]],
): { selectedId: Id; tabs: { id: Id; label: string; href: string }[] } {
  const { tab } = useParams();
  const selected = tabs.find(({ id }) => id === tab) ?? tabs[0];
  return {
    selectedId: selected.id,
    tabs: tabs.map(({ id, label }) => ({ id, label, href: `${basePath}/${id}` })),
  };
}
