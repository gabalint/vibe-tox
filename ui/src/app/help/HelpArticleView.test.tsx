import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { type ManualPage, normalizeArticle, pageForPath } from "./articles";
import { HelpArticleView } from "./HelpArticleView";

const legacyTopic = {
  id: "synthetic-help",
  title: "Synthetic help",
  summary: "Guidance for this task.",
  reviewedOn: "2026-10-14",
  routes: ["/synthetic", "/synthetic/:recordId"],
  sections: [
    {
      title: "Before starting",
      paragraphs: ["Review the supplied information."],
      steps: ["Open the record."],
    },
  ],
};

const recordPage: ManualPage = {
  title: "Record reference",
  routes: ["/synthetic/:recordId"],
  purpose: "Review the selected record.",
  access: "An authorized session is required.",
  fields: [],
  actions: [],
  restrictions: ["Synthetic records only."],
  workflow: ["Review the record."],
  troubleshooting: [],
};

describe("Help article format compatibility", () => {
  it("shows legacy sections immediately without inventing a page reference or review metadata", () => {
    const article = normalizeArticle(legacyTopic);
    expect(article.pages).toEqual([]);
    expect(article.reviewSources).toEqual([]);
    expect(pageForPath(article, "/synthetic")).toBeUndefined();
    render(<HelpArticleView article={article} pathname="/synthetic" />);
    expect(screen.getByRole("heading", { name: "Before starting" })).toBeTruthy();
    expect(screen.getByText("Review the supplied information.").closest("details")).toHaveProperty(
      "open",
      true,
    );
    expect(screen.getByRole("listitem").textContent).toBe("Open the record.");
  });

  it("preserves per-page route selection and keeps background guidance separately expandable", () => {
    const source = {
      ...legacyTopic,
      reviewSources: ["ui/src/app/help/HelpArticleView.tsx"],
      pages: [
        {
          ...recordPage,
          title: "List reference",
          routes: ["/synthetic"],
          purpose: "Find a record.",
        },
        recordPage,
      ],
    };
    const article = normalizeArticle(source);
    expect(article.pages).toBe(source.pages);
    expect(article.reviewSources).toBe(source.reviewSources);
    expect(pageForPath(article, "/synthetic/example")).toBe(recordPage);
    render(<HelpArticleView article={article} pathname="/synthetic/example" />);
    expect(screen.getByText("Review the selected record.")).toBeTruthy();
    expect(screen.queryByText("Find a record.")).toBeNull();
    expect(screen.getByText("Review the supplied information.").closest("details")).toHaveProperty(
      "open",
      false,
    );
  });
});
