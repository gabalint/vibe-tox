import { act, screen } from "@testing-library/react";
import { useParams } from "react-router";
import { describe, expect, it } from "vitest";
import { Button } from "@/components/Button";
import { PageHeader } from "@/components/Page";
import { useOpenRecordPage, useRecordSequence } from "./recordSequence";
import { renderRoutes } from "./test/router";
import { expectNextKeepsFocus, renderFromList } from "./test/sequence";

const recordPath = (id: string) => `/runs/${id}`;
const IDS = ["SYN-R1", "SYN-R2", "SYN-R3"];
const HOME = { label: "Runs", href: "/runs" };

/** Another list that opens the same records, such as a work queue. */
function QueuePage() {
  const open = useOpenRecordPage(recordPath, "Runs to close");
  return <Button onPress={() => open("SYN-R2", IDS)}>Open SYN-R2</Button>;
}

function ListPage() {
  const open = useOpenRecordPage(recordPath, "Runs");
  return <Button onPress={() => open("SYN-R2", IDS)}>Open SYN-R2</Button>;
}

function RecordPage() {
  const { id = "" } = useParams();
  const { list, sequence } = useRecordSequence(id, HOME, recordPath);
  return (
    <PageHeader
      title={id}
      breadcrumbs={[list, { label: id, href: recordPath(id) }]}
      sequence={sequence}
    />
  );
}

const ROUTES = [
  { path: "/runs", element: <ListPage /> },
  { path: "/runs/:id", element: <RecordPage /> },
  { path: "/queue", element: <QueuePage /> },
];

const heading = () => screen.getByRole("heading", { level: 1 }).textContent;
const link = (name: string) => screen.getByRole("link", { name });

describe("recordSequence", () => {
  it("opens a record with the list's address and order, and steps through it", async () => {
    const { user, router } = renderRoutes(ROUTES, "/runs?q=syn&status=open");
    await user.click(screen.getByRole("button", { name: "Open SYN-R2" }));
    expect(heading()).toBe("SYN-R2");
    expect(link("Runs").getAttribute("href")).toBe("/runs?q=syn&status=open");

    await user.click(link("Next"));
    expect(heading()).toBe("SYN-R3");
    expect(link("Next").getAttribute("aria-disabled")).toBe("true");

    await user.click(link("Previous"));
    await user.click(link("Previous"));
    expect(heading()).toBe("SYN-R1");
    expect(link("Previous").getAttribute("aria-disabled")).toBe("true");
    // Previous and Next replace the entry, so Back returns to the list as it was.
    await act(() => router.navigate(-1));
    expect(router.state.location.pathname).toBe("/runs");
    expect(router.state.location.search).toBe("?q=syn&status=open");
  });

  it("keeps focus on Next through the list, onto its last record", async () => {
    const { user } = await renderFromList({
      route: { path: "/runs/:id", element: <RecordPage /> },
      recordPath,
      ids: IDS,
    });
    expect(heading()).toBe("SYN-R1");
    await expectNextKeepsFocus(user, "SYN-R2");
    await expectNextKeepsFocus(user, "SYN-R3");
    expect(link("Next").getAttribute("aria-disabled")).toBe("true");
    expect(link("Synthetic list").getAttribute("href")).toBe("/list");
  });

  it("goes back to the list's find and filters from the breadcrumb", async () => {
    const { user, router } = renderRoutes(ROUTES, "/runs?q=syn");
    await user.click(screen.getByRole("button", { name: "Open SYN-R2" }));
    await user.click(link("Runs"));
    expect(router.state.location.search).toBe("?q=syn");
  });

  it("names the list the record was opened from in the breadcrumb", async () => {
    const { user } = renderRoutes(ROUTES, "/queue");
    await user.click(screen.getByRole("button", { name: "Open SYN-R2" }));
    expect(link("Runs to close").getAttribute("href")).toBe("/queue");
  });

  it("without the list's state, links to the list's base path and offers no Previous or Next", () => {
    renderRoutes(ROUTES, "/runs/SYN-R2");
    expect(heading()).toBe("SYN-R2");
    expect(link("Runs").getAttribute("href")).toBe("/runs");
    expect(screen.queryByRole("navigation", { name: "Records in list order" })).toBeNull();
  });

  it("offers no Previous or Next for a record that is not in the list it came from", async () => {
    const { user, router } = renderRoutes(ROUTES, "/runs");
    await user.click(screen.getByRole("button", { name: "Open SYN-R2" }));
    await act(() => router.navigate("/runs/SYN-R9", { state: router.state.location.state }));
    expect(heading()).toBe("SYN-R9");
    expect(link("Runs").getAttribute("href")).toBe("/runs");
    expect(screen.queryByRole("link", { name: "Next" })).toBeNull();
  });
});
