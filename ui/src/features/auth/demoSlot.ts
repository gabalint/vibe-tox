import { type ComponentType, createContext } from "react";

/**
 * The way into the demo laboratory offered on the setup screen (proposed REQ-OPS-016). It
 * belongs to the demo feature, which auth may not import, so the app frame provides it here;
 * without a provider the setup screen offers none.
 */
export const DemoEntryContext = createContext<ComponentType | null>(null);
