// What a Standard Catalog selection implies, for display only (proposed REQ-OPS-014). The
// engine computes the same closure from the explicit choices and refuses anything unavailable;
// the UI sends only the choices.
import type { LoadStandardCatalog } from "@/generated/LoadStandardCatalog";
import type { StandardCatalogTree } from "@/generated/StandardCatalogTree";
import type { StandardComponentItem } from "@/generated/StandardComponentItem";
import type { StandardItemState } from "@/generated/StandardItemState";
import type { StandardMethodItem } from "@/generated/StandardMethodItem";

/** The person's explicit choices. */
export interface Choices {
  /** Tests and panels chosen, by code. */
  orderables: ReadonlySet<string>;
  /** Methods chosen on their own, by code. */
  methods: ReadonlySet<string>;
  /** Analytes removed from selected methods, as `omissionKey(method, analyte)`. */
  omitted: ReadonlySet<string>;
}

export const NO_CHOICES: Choices = {
  orderables: new Set(),
  methods: new Set(),
  omitted: new Set(),
};

/** Codes hold no spaces, so a space separates the method and the analyte. */
export function omissionKey(methodCode: string, analyteCode: string): string {
  return `${methodCode} ${analyteCode}`;
}

interface Stateful {
  state: StandardItemState;
  reason: string | null;
}

/** Why an item cannot be chosen, in the screen's words; null when it can. */
export function unavailableText(item: Stateful): string | null {
  switch (item.state) {
    case "new":
      return null;
    case "already_present":
      return "Already in your catalog";
    case "code_in_use":
      return item.reason ?? "Code in use";
    case "blocked":
      return item.reason ?? "Cannot be loaded";
  }
}

/** An item the selection needs that cannot be loaded: the load is refused until it is cleared. */
interface Blocker {
  code: string;
  name: string;
  reason: string;
}

/** What the selection loads, per kind, and what it needs that is already present. */
export interface Counts {
  specimenTypes: number;
  analytes: number;
  methods: number;
  tests: number;
  panels: number;
  /** Specimen types and analytes needed but already in the catalog; not loaded again. */
  skipped: number;
}

export interface Selection {
  /** Methods selected, chosen or implied. */
  methods: ReadonlySet<string>;
  /** For each implied method, the names of the chosen tests and panels that need it. */
  methodNeeders: ReadonlyMap<string, readonly string[]>;
  /** For each needed analyte, the names of the selected methods that keep it. */
  analyteNeeders: ReadonlyMap<string, readonly string[]>;
  /** For each needed specimen type, the names of the selected methods on it. */
  specimenTypeNeeders: ReadonlyMap<string, readonly string[]>;
  counts: Counts;
  blockers: readonly Blocker[];
}

function addNeeder(map: Map<string, string[]>, code: string, name: string) {
  const names = map.get(code);
  if (names) names.push(name);
  else map.set(code, [name]);
}

/** The chosen tests and panels that can be loaded, in file order. */
function chosenOrderables(tree: StandardCatalogTree, choices: Choices) {
  return tree.orderables.filter((o) => o.state === "new" && choices.orderables.has(o.code));
}

/** The methods the choices select: chosen on their own, or needed by a chosen test or panel. */
function impliedMethods(tree: StandardCatalogTree, choices: Choices) {
  const needers = new Map<string, string[]>();
  for (const orderable of chosenOrderables(tree, choices)) {
    for (const code of orderable.methodCodes) addNeeder(needers, code, orderable.name);
  }
  const selected = new Set<string>();
  for (const method of tree.methods) {
    const chosen = method.state === "new" && choices.methods.has(method.code);
    if (chosen || needers.has(method.code)) selected.add(method.code);
  }
  return { selected, needers };
}

/** The analytes a selected method keeps after removals. */
function keptAnalytes(method: StandardMethodItem, choices: Choices): string[] {
  return method.analytes
    .map((a) => a.analyteCode)
    .filter((code) => !choices.omitted.has(omissionKey(method.code, code)));
}

type Item = Stateful & { code: string; name: string };

/** Sorts the closure's items of one kind into created, skipped and blocking. */
function tally(
  items: readonly Item[],
  needed: (code: string) => boolean,
  kind: "specimenTypes" | "analytes" | "methods",
  counts: Counts,
  blockers: Blocker[],
) {
  for (const item of items.filter((each) => needed(each.code))) {
    if (item.state === "new") counts[kind] += 1;
    else if (item.state === "already_present") counts.skipped += 1;
    else blockers.push({ code: item.code, name: item.name, reason: unavailableText(item) ?? "" });
  }
}

/** The selection's closure, counts and blockers. */
export function deriveSelection(tree: StandardCatalogTree, choices: Choices): Selection {
  const { selected, needers } = impliedMethods(tree, choices);
  const analyteNeeders = new Map<string, string[]>();
  const specimenTypeNeeders = new Map<string, string[]>();
  for (const method of tree.methods.filter((m) => selected.has(m.code))) {
    addNeeder(specimenTypeNeeders, method.specimenTypeCode, method.name);
    for (const code of keptAnalytes(method, choices)) addNeeder(analyteNeeders, code, method.name);
  }
  const counts: Counts = {
    specimenTypes: 0,
    analytes: 0,
    methods: 0,
    tests: 0,
    panels: 0,
    skipped: 0,
  };
  const blockers: Blocker[] = [];
  tally(
    tree.specimenTypes,
    (code) => specimenTypeNeeders.has(code),
    "specimenTypes",
    counts,
    blockers,
  );
  tally(tree.analytes, (code) => analyteNeeders.has(code), "analytes", counts, blockers);
  tally(tree.methods, (code) => selected.has(code), "methods", counts, blockers);
  for (const orderable of chosenOrderables(tree, choices)) {
    if (orderable.kind === "panel") counts.panels += 1;
    else counts.tests += 1;
  }
  return {
    methods: selected,
    methodNeeders: needers,
    analyteNeeders,
    specimenTypeNeeders,
    counts,
    blockers,
  };
}

/** Drops removals from methods no longer selected, so choosing a method again starts complete. */
function pruned(tree: StandardCatalogTree, choices: Choices): Choices {
  const { selected } = impliedMethods(tree, choices);
  const omitted = new Set(
    [...choices.omitted].filter((key) => selected.has(key.split(" ")[0] ?? "")),
  );
  return { ...choices, omitted };
}

function toggled(set: ReadonlySet<string>, code: string, on: boolean): ReadonlySet<string> {
  const next = new Set(set);
  if (on) next.add(code);
  else next.delete(code);
  return next;
}

export function chooseOrderable(
  tree: StandardCatalogTree,
  choices: Choices,
  code: string,
  on: boolean,
): Choices {
  return pruned(tree, { ...choices, orderables: toggled(choices.orderables, code, on) });
}

export function chooseMethod(
  tree: StandardCatalogTree,
  choices: Choices,
  code: string,
  on: boolean,
): Choices {
  return pruned(tree, { ...choices, methods: toggled(choices.methods, code, on) });
}

/** Keeps or removes one analyte of a selected method. */
export function keepAnalyte(
  choices: Choices,
  methodCode: string,
  analyteCode: string,
  keep: boolean,
): Choices {
  const key = omissionKey(methodCode, analyteCode);
  return { ...choices, omitted: toggled(choices.omitted, key, !keep) };
}

/** The components of chosen tests and panels that measure `methodCode`, with their owner. */
function componentsOf(tree: StandardCatalogTree, choices: Choices, methodCode: string) {
  return chosenOrderables(tree, choices).flatMap((orderable) =>
    orderable.components
      .filter((c) => c.methodCode === methodCode)
      .map((component) => ({ orderable, component })),
  );
}

/**
 * Why `analyteCode` cannot be removed from the selected method: the method, or a component of
 * a chosen test or panel, would be left without analytes. Null when it can.
 */
export function removalBlock(
  tree: StandardCatalogTree,
  choices: Choices,
  method: StandardMethodItem,
  analyteCode: string,
): string | null {
  const after = keepAnalyte(choices, method.code, analyteCode, false);
  if (keptAnalytes(method, after).length === 0) {
    return `${method.name} needs at least one analyte; this is its last one.`;
  }
  for (const { orderable, component } of componentsOf(tree, after, method.code)) {
    if (componentAnalytes(component, after).every((a) => a.removed)) {
      return `${orderable.name} needs at least one analyte from ${method.name}; this is its last one.`;
    }
  }
  return null;
}

/** The names of the chosen tests and panels that lose `analyteCode` with the method's removal. */
export function alsoRemovedFrom(
  tree: StandardCatalogTree,
  choices: Choices,
  methodCode: string,
  analyteCode: string,
): string[] {
  return componentsOf(tree, choices, methodCode)
    .filter(({ component }) => component.analyteCodes.includes(analyteCode))
    .map(({ orderable }) => orderable.name);
}

/** A component's analytes, each marked when removed from its method. */
export function componentAnalytes(component: StandardComponentItem, choices: Choices) {
  return component.analyteCodes.map((code) => ({
    code,
    removed: choices.omitted.has(omissionKey(component.methodCode, code)),
  }));
}

/** The load command's input: the explicit choices only, after the disclaimer. */
export function toLoadInput(
  tree: StandardCatalogTree,
  choices: Choices,
  disclaimerAcknowledged: boolean,
): LoadStandardCatalog {
  const current = pruned(tree, choices);
  return {
    catalogVersion: tree.info.catalogVersion,
    orderableCodes: tree.orderables
      .filter((o) => current.orderables.has(o.code))
      .map((o) => o.code),
    methodCodes: tree.methods.filter((m) => current.methods.has(m.code)).map((m) => m.code),
    omittedAnalytes: [...current.omitted].map((key) => {
      const [methodCode = "", analyteCode = ""] = key.split(" ");
      return { methodCode, analyteCode };
    }),
    disclaimerAcknowledged,
  };
}

/** The file's name for an item code; the code itself if the file has no such item. */
export function nameOf(items: readonly { code: string; name: string }[], code: string): string {
  return items.find((item) => item.code === code)?.name ?? code;
}
