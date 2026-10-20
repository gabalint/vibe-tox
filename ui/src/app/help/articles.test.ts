import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { detailScreens } from "../detailScreens";
import { navigation } from "../navigation";
import { articleFiles, articleForPath, articles, findArticles, pageForPath } from "./articles";

const screenRoutes = [...navigation.flatMap((section) => section.screens), ...detailScreens].map(
  ({ path, tabbed }) => (tabbed ? `${path}/:tab?` : path),
);

function nonempty(value: unknown) {
  expect(typeof value).toBe("string");
  expect((value as string).trim()).not.toBe("");
}

function referenceEntries(entries: unknown, keys: string[], allowEmpty = false) {
  expect(Array.isArray(entries)).toBe(true);
  const rows = entries as Record<string, string>[];
  if (!allowEmpty) expect(rows.length).toBeGreaterThan(0);
  for (const entry of rows) {
    expect(Object.keys(entry).sort()).toEqual([...keys].sort());
    for (const key of keys) nonempty(entry[key]);
  }
  const names = rows.map((entry) => entry[keys[0] ?? "name"]);
  expect(new Set(names).size).toBe(rows.length);
}

describe("Bundled page documentation", () => {
  it("documents every signed-in screen explicitly, without stale or duplicate route mappings", () => {
    const documentedRoutes = articles.flatMap((article) => article.routes);
    expect([...documentedRoutes].sort()).toEqual([...screenRoutes].sort());
    expect(new Set(documentedRoutes).size).toBe(documentedRoutes.length);
    expect(articles.some((article) => article.id === "getting-started")).toBe(true);
  });

  it("keeps authoring metadata and readable content valid", () => {
    expect(new Set(articles.map((article) => article.id)).size).toBe(articles.length);
    for (const [file, article] of Object.entries(articleFiles)) {
      expect(file.endsWith(`/${article.id}.json`)).toBe(true);
      expect(article.id).toMatch(/^[a-z]+(?:-[a-z]+)*$/);
      expect(article.title.trim()).not.toBe("");
      expect(article.summary.trim()).not.toBe("");
      expect(article.reviewedOn).toMatch(/^\d{4}-\d{2}-\d{2}$/);
      expect(new Date(article.reviewedOn).toISOString().slice(0, 10)).toBe(article.reviewedOn);
      expect(article.sections.length).toBeGreaterThan(0);
      expect(new Set(article.sections.map((section) => section.title)).size).toBe(
        article.sections.length,
      );
      for (const section of article.sections) {
        expect(section.title.trim()).not.toBe("");
        const content = [...section.paragraphs, ...(section.steps ?? [])];
        expect(content.length).toBeGreaterThan(0);
        for (const text of content) expect(text.trim()).not.toBe("");
      }
    }
  });

  it.each(articles)("$id names the existing source files reviewed", (article) => {
    expect(article.reviewSources.length).toBeGreaterThan(0);
    expect(new Set(article.reviewSources).size).toBe(article.reviewSources.length);
    for (const source of article.reviewSources) {
      expect(source).toMatch(/^(ui\/src|crates|src-tauri\/src)\//);
      expect(source.split("/")).not.toContain("..");
      expect(readFileSync(source, "utf8").length, source).toBeGreaterThan(0);
    }
  });

  it.each(articles)("$id has a detailed reference and a traceable source inventory", (article) => {
    expect(article.pages.length).toBeGreaterThan(0);
    expect(new Set(article.pages.map((page) => page.title)).size).toBe(article.pages.length);
    expect(article.pages.flatMap((page) => page.routes).sort()).toEqual([...article.routes].sort());
    for (const page of article.pages) {
      for (const text of [page.title, page.purpose, page.access]) nonempty(text);
      if (article.id !== "getting-started") expect(page.routes.length).toBeGreaterThan(0);
      referenceEntries(page.fields, ["name", "meaning", "values", "rules"]);
      referenceEntries(page.actions, ["name", "does", "availableWhen", "result"], true);
      referenceEntries(page.troubleshooting, ["problem", "recovery"]);
      for (const list of [page.restrictions, page.workflow]) {
        expect(list.length).toBeGreaterThan(0);
        for (const text of list) nonempty(text);
      }
    }
  });

  it.each(screenRoutes)("resolves %s to its own article, not the fallback", (pattern) => {
    const path = pattern.replace(/:[^/]+\?/g, "history").replace(/:[^/]+/g, "synthetic-id");
    expect(articleForPath(path)?.routes).toContain(pattern);
    const article = articleForPath(path);
    if (article) expect(pageForPath(article, path)?.routes).toContain(pattern);
    if (pattern.endsWith("/:tab?")) {
      expect(articleForPath(path.slice(0, path.lastIndexOf("/")))?.routes).toContain(pattern);
    }
  });

  it("ranks static and nested routes ahead of record routes and falls back for unknown pages", () => {
    expect(articleForPath("/orders/new")?.routes).toContain("/orders/new");
    expect(articleForPath("/orders/synthetic-id/accession")?.routes).toContain(
      "/orders/:orderId/accession",
    );
    expect(articleForPath("/unknown-page")?.id).toBe("getting-started");
  });

  it("searches article text case-insensitively and handles empty or unmatched searches", () => {
    expect(findArticles("  ")).toEqual(articles);
    expect(findArticles("BACKUP").length).toBeGreaterThan(0);
    expect(findArticles("zzzz-no-such-topic")).toEqual([]);
    // Search reaches the detailed reference, not only its title or introductory summary.
    const manual = articles.find((article) => article.id === "getting-started");
    expect(findArticles("32 pixels")).toContain(manual);
  });
});
