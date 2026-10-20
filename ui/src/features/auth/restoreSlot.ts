import { type ComponentType, createContext } from "react";

/**
 * The restore-from-backup flow offered on the setup screen. It belongs to the operations
 * feature, which auth may not import, so the app frame provides it here; without a provider the
 * setup screen offers no restore.
 */
export const RestoreFlowContext = createContext<ComponentType<{ onExit: () => void }> | null>(null);
