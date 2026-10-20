import type { CommandName } from "@/generated/CommandName";
import { useCommandMutation } from "@/lib/commands";
import { ACCESSION_QUERIES } from "./contract";

/** An accessioning command; it refreshes the accession and container views. */
export function useAccessionCommand<Input, Result = unknown>(command: CommandName) {
  return useCommandMutation<Result, { input: Input }>(command, {
    invalidates: ACCESSION_QUERIES,
  });
}
