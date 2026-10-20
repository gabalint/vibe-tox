import { fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useRef } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { HelpSizeMenu, HelpSplitter } from "./HelpResize";
import { useHelpLayout } from "./useHelpLayout";

const STORAGE_KEY = "vibe-tox.help.layout-v1";

function Harness() {
  const main = useRef<HTMLElement>(null);
  const pane = useRef<HTMLDivElement>(null);
  const layout = useHelpLayout(main, pane, true);
  return (
    <>
      <main ref={main} />
      <div ref={pane} id="test-help">
        <HelpSplitter layout={layout} controls="test-help" />
        <HelpSizeMenu layout={layout} />
      </div>
    </>
  );
}

function size() {
  return Number(screen.getByRole("separator").getAttribute("aria-valuenow"));
}

function pointer(target: HTMLElement, type: string, x: number) {
  const event = new MouseEvent(type, { bubbles: true, button: 0, clientX: x });
  Object.defineProperties(event, { pointerId: { value: 1 }, isPrimary: { value: true } });
  fireEvent(target, event);
}

describe("Resizable user manual", () => {
  beforeEach(() => localStorage.clear());
  afterEach(() => {
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
  });

  it("resizes in the physical arrow direction, clamps bounds and remembers size across mounts", async () => {
    const user = userEvent.setup();
    const first = render(<Harness />);
    const divider = screen.getByRole("separator", { name: "Page help width" });
    expect(divider.getAttribute("aria-controls")).toBe("test-help");
    expect(size()).toBe(400);
    divider.focus();
    await user.keyboard("{ArrowLeft}");
    expect(size()).toBe(432);
    first.unmount();
    render(<Harness />);
    expect(size()).toBe(432);
    screen.getByRole("separator").focus();
    await user.keyboard("{Home}{ArrowRight}");
    expect(size()).toBe(320);
    await user.keyboard("{End}{ArrowLeft}");
    expect(size()).toBe(520);
  });

  it("offers smaller, larger and reset without dragging", async () => {
    const user = userEvent.setup();
    render(<Harness />);
    const options = screen.getByRole("button", { name: "Page help size options" });
    await user.click(options);
    await user.click(await screen.findByRole("menuitem", { name: "Wider panel" }));
    expect(size()).toBe(432);
    await user.click(options);
    await user.click(await screen.findByRole("menuitem", { name: "Narrower panel" }));
    expect(size()).toBe(400);
    screen.getByRole("separator").focus();
    await user.keyboard("{End}");
    await user.click(options);
    await user.click(await screen.findByRole("menuitem", { name: "Reset panel size" }));
    expect(size()).toBe(400);
  });

  it("uses independent bottom-panel height and reclamps after available space changes", async () => {
    const user = userEvent.setup();
    localStorage.setItem(STORAGE_KEY, JSON.stringify({ width: 464, height: 352 }));
    vi.stubGlobal("matchMedia", () => ({
      matches: true,
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
    }));
    const first = render(<Harness />);
    const divider = screen.getByRole("separator", { name: "Page help height" });
    expect(divider.getAttribute("aria-orientation")).toBe("horizontal");
    expect(size()).toBe(352);
    divider.focus();
    await user.keyboard("{ArrowUp}");
    expect(size()).toBe(384);
    expect(JSON.parse(localStorage.getItem(STORAGE_KEY) ?? "{}").width).toBe(464);
    // The available height excludes the header/notices; preserve 40% for the working page.
    vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockReturnValue({
      height: 200,
    } as DOMRect);
    fireEvent(window, new Event("resize"));
    expect(size()).toBe(240);
    first.unmount();
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
    render(<Harness />);
    expect(size()).toBe(464);
  });

  it("restores heights above 1000 pixels when a tall window has room", () => {
    vi.stubGlobal("matchMedia", () => ({
      matches: true,
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
    }));
    vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockReturnValue({
      height: 1000,
    } as DOMRect);
    localStorage.setItem(STORAGE_KEY, JSON.stringify({ width: 400, height: 1100 }));
    render(<Harness />);
    expect(size()).toBe(1100);
  });

  it("captures a drag, saves only at completion and restores a cancelled drag", () => {
    render(<Harness />);
    const divider = screen.getByRole("separator");
    const captured = new Set<number>();
    divider.setPointerCapture = (id) => {
      captured.add(id);
    };
    divider.hasPointerCapture = (id) => captured.has(id);
    divider.releasePointerCapture = (id) => {
      captured.delete(id);
    };
    pointer(divider, "pointerdown", 800);
    pointer(divider, "pointermove", 720);
    expect(size()).toBe(480);
    expect(JSON.parse(localStorage.getItem(STORAGE_KEY) ?? "{}").width).toBe(400);
    pointer(divider, "pointercancel", 720);
    expect(size()).toBe(400);
    expect(captured.size).toBe(0);
    pointer(divider, "pointerdown", 800);
    pointer(divider, "pointermove", 736);
    pointer(divider, "pointerup", 736);
    expect(size()).toBe(464);
    expect(JSON.parse(localStorage.getItem(STORAGE_KEY) ?? "{}").width).toBe(464);
  });

  it.each(["{broken", "null", '{"width":"wide","height":-50}'])(
    "tolerates invalid preferences: %s",
    (saved) => {
      localStorage.setItem(STORAGE_KEY, saved);
      render(<Harness />);
      expect(size()).toBe(400);
    },
  );

  it("keeps resizing usable when preference storage is unavailable", async () => {
    vi.spyOn(Storage.prototype, "getItem").mockImplementation(() => {
      throw new Error("Unavailable");
    });
    vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => {
      throw new Error("Unavailable");
    });
    render(<Harness />);
    screen.getByRole("separator").focus();
    await userEvent.setup().keyboard("{ArrowLeft}");
    expect(size()).toBe(432);
  });
});
