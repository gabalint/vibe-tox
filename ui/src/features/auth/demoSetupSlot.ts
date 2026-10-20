import { type ComponentType, createContext } from "react";

/** The app supplies demo preparation without coupling auth to the demo feature. */
export const DemoSetupContext = createContext<ComponentType | null>(null);
