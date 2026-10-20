import { screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { renderApp } from "../frameTestData";
import { articleForPath, pageForPath } from "./articles";

describe("Page help in the application frame", () => {
  // NFR-09: disclosure semantics, keyboard access, focus restoration, nonmodal reading.
  it("opens from the keyboard, allows access to the page and restores focus on Escape", async () => {
    const { user, container } = renderApp("/about", []);
    const trigger = await screen.findByRole("button", { name: "Page help" });
    expect(trigger.getAttribute("aria-expanded")).toBe("false");
    expect(screen.queryByRole("complementary", { name: "Page help" })).toBeNull();
    trigger.focus();
    await user.keyboard("{Enter}");
    const help = await screen.findByRole("complementary", { name: "Page help" });
    expect(trigger.getAttribute("aria-expanded")).toBe("true");
    expect(document.activeElement).toBe(within(help).getByRole("heading", { name: "Page help" }));
    expect(screen.getByRole("main")).toBeTruthy();
    expect(screen.queryByRole("dialog")).toBeNull();
    await expectNoA11yViolations(container, { wholePage: true });
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("complementary", { name: "Page help" })).toBeNull();
    expect(document.activeElement).toBe(trigger);
  });

  it("searches topics without navigating, preserves selection on reopen and resets on navigation", async () => {
    const { user, router } = renderApp("/about", []);
    await user.click(await screen.findByRole("button", { name: "Page help" }));
    let help = await screen.findByRole("complementary", { name: "Page help" });
    await user.click(within(help).getByRole("button", { name: "Browse all topics" }));
    const search = within(help).getByRole("searchbox", { name: "Find help topics" });
    await user.type(search, "zzzz-no-such-topic{Enter}");
    expect(await within(help).findByText(/No help topics match/)).toBeTruthy();
    await user.clear(search);
    const guide = articleForPath("/operations/backups");
    await user.type(search, `${guide?.title}{Enter}`);
    await user.click(await within(help).findByRole("button", { name: guide?.title }));
    expect(within(help).getByRole("heading", { name: guide?.title })).toBeTruthy();
    expect(router.state.location.pathname).toBe("/about");
    await user.click(within(help).getByRole("button", { name: "Close page help" }));
    await user.click(screen.getByRole("button", { name: "Page help" }));
    help = screen.getByRole("complementary", { name: "Page help" });
    expect(within(help).getByRole("heading", { name: guide?.title })).toBeTruthy();
    await user.click(screen.getByRole("link", { name: "Work queues" }));
    const heading = await screen.findByRole("heading", { level: 1, name: "Work queues" });
    await waitFor(() => expect(document.activeElement).toBe(heading));
    expect(
      within(help).getByRole("heading", { name: articleForPath("/work-queues")?.title }),
    ).toBeTruthy();
  });

  it("preserves typed input and the current URL when help is opened and closed", async () => {
    const { user, router } = renderApp("/patients/register", ["enter_orders"]);
    const input = await screen.findByRole("textbox", { name: "Family name" });
    await user.type(input, "SYNTHETIC-UNSUBMITTED");
    const location = router.state.location;
    await user.click(screen.getByRole("button", { name: "Page help" }));
    const divider = await screen.findByRole("separator", { name: "Page help width" });
    divider.focus();
    await user.keyboard("{ArrowLeft}");
    expect(input).toHaveProperty("value", "SYNTHETIC-UNSUBMITTED");
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("complementary", { name: "Page help" })).toBeNull();
    expect(input).toHaveProperty("value", "SYNTHETIC-UNSUBMITTED");
    expect(screen.getByRole("textbox", { name: "Family name" })).toBe(input);
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(router.state.location).toEqual(location);
  });

  it("opens the exact page reference, exposes field details and resets a related-page selection", async () => {
    const { user, router } = renderApp("/patients/register", ["enter_orders"]);
    await user.click(await screen.findByRole("button", { name: "Page help" }));
    const help = await screen.findByRole("complementary", { name: "Page help" });
    const article = articleForPath("/patients/register");
    if (!article) throw new Error("Patient manual missing");
    const page = pageForPath(article, "/patients/register");
    if (!page) throw new Error("Registration manual missing");
    const selector = within(help).getByRole("button", { name: /Page in this topic/ });
    expect(selector.textContent).toContain(page.title);
    const field = page.fields[0];
    if (!field) throw new Error("Registration fields missing");
    await user.click(within(help).getByText(field.name, { selector: "summary" }));
    const table = within(help).getByRole("table", { name: field.name });
    expect(within(table).getByText(field.meaning)).toBeTruthy();
    expect(within(table).getByText(field.values)).toBeTruthy();
    expect(within(table).getByText(field.rules)).toBeTruthy();
    await expectNoA11yViolations(help);
    const related = article.pages.find((entry) => entry.title !== page.title);
    if (!related) throw new Error("Related patient reference missing");
    await user.click(selector);
    await user.click(await screen.findByRole("option", { name: related.title }));
    expect(selector.textContent).toContain(related.title);
    await user.click(within(help).getByRole("button", { name: "Current page" }));
    expect(within(help).getByRole("button", { name: /Page in this topic/ }).textContent).toContain(
      page.title,
    );
    expect(router.state.location.pathname).toBe("/patients/register");
  });
});
