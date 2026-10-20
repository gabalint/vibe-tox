import { matchRoutes } from "react-router";

export interface ManualPage {
  title: string;
  routes: string[];
  purpose: string;
  access: string;
  fields: { name: string; meaning: string; values: string; rules: string }[];
  actions: { name: string; does: string; availableWhen: string; result: string }[];
  restrictions: string[];
  workflow: string[];
  troubleshooting: { problem: string; recovery: string }[];
}

export interface HelpArticle {
  id: string;
  title: string;
  summary: string;
  reviewedOn: string;
  routes: string[];
  reviewSources: string[];
  pages: ManualPage[];
  sections: { title: string; paragraphs: string[]; steps?: string[] }[];
}

type HelpArticleSource = Omit<HelpArticle, "pages" | "reviewSources"> &
  Partial<Pick<HelpArticle, "pages" | "reviewSources">>;

/** Keep legacy bundled topics readable while their detailed references are authored. */
export function normalizeArticle(article: HelpArticleSource): HelpArticle {
  return { ...article, pages: article.pages ?? [], reviewSources: article.reviewSources ?? [] };
}

export function pageForPath(article: HelpArticle, pathname: string): ManualPage | undefined {
  const routes = article.pages.flatMap((page) => page.routes.map((path) => ({ path, page })));
  return matchRoutes(routes, pathname)?.at(-1)?.route.page ?? article.pages[0];
}

// NFR-31: documentation ships with the app; no remote content or HTML renderer.
const bundledArticles = import.meta.glob<HelpArticleSource>("../../../../docs/help/*.json", {
  eager: true,
  import: "default",
});
export const articleFiles = Object.fromEntries(
  Object.entries(bundledArticles).map(([path, article]) => [path, normalizeArticle(article)]),
);

export const articles = Object.values(articleFiles).sort((a, b) => a.title.localeCompare(b.title));
const helpRoutes = articles.flatMap((article) => article.routes.map((path) => ({ path, article })));

/** Use the router's own ranking, so `/orders/new` wins over `/orders/:orderId`. */
export function articleForPath(pathname: string): HelpArticle | undefined {
  return (
    matchRoutes(helpRoutes, pathname)?.at(-1)?.route.article ??
    articles.find((article) => article.id === "getting-started")
  );
}

export function findArticles(query: string): HelpArticle[] {
  const words = query.trim().toLocaleLowerCase().split(/\s+/);
  return articles.filter((article) => {
    const text = [
      article.title,
      article.summary,
      ...article.pages.flatMap((page) => [
        page.title,
        page.purpose,
        page.access,
        ...page.fields.flatMap((field) => [field.name, field.meaning, field.values, field.rules]),
        ...page.actions.flatMap((action) => [
          action.name,
          action.does,
          action.availableWhen,
          action.result,
        ]),
        ...page.restrictions,
        ...page.workflow,
        ...page.troubleshooting.flatMap((entry) => [entry.problem, entry.recovery]),
      ]),
      ...article.sections.flatMap((section) => [
        section.title,
        ...section.paragraphs,
        ...(section.steps ?? []),
      ]),
    ]
      .join(" ")
      .toLocaleLowerCase();
    return words.every((word) => text.includes(word));
  });
}
