import {
  type InfiniteData,
  keepPreviousData,
  MutationCache,
  QueryCache,
  QueryClient,
  type QueryKey,
  useInfiniteQuery,
  useMutation,
  useQuery,
  useQueryClient,
} from "@tanstack/react-query";
import type { InvokeArgs } from "@tauri-apps/api/core";
import type { CommandError } from "@/generated/CommandError";
import type { CommandName } from "@/generated/CommandName";
import type { SessionInfo } from "@/generated/SessionInfo";
import { call } from "./ipc";
import { type AuthenticationStatus, securityKeySelection } from "./securityKey";

declare module "@tanstack/react-query" {
  interface Register {
    defaultError: CommandError;
  }
}

/** `current_session() → SessionInfo | null`: who is signed in, and whether the session is locked. */
export const CURRENT_SESSION = "current_session" satisfies CommandName;

/**
 * `get_laboratory_profile() → LaboratoryProfileView | null`: the laboratory's profile, whose
 * time zone every screen shows local times in. Any signed-in session may read it.
 */
export const GET_LABORATORY_PROFILE = "get_laboratory_profile" satisfies CommandName;

/** Answers meaning the session locked or ended since the screen was shown. */
const SESSION_ENDED_CODES: ReadonlySet<string> = new Set(["session_locked", "unauthenticated"]);

/**
 * Engine state cache. Commands run in-process, so a failure is an answer, not a network
 * blip: nothing is retried automatically. Nothing is refetched on window focus either: many
 * reads are audited accesses to patient data, recorded each time they run. Mutations
 * refresh what they change. When any command reports a locked or ended
 * session, the session is asked for again, so the lock or sign-in screen replaces the app
 * whichever screen noticed it (REQ-IDN-004).
 */
export function createQueryClient(): QueryClient {
  const recheckSession = (error: CommandError) => {
    if (SESSION_ENDED_CODES.has(error.code)) {
      void client.invalidateQueries({ queryKey: [CURRENT_SESSION] });
    }
  };
  const client = new QueryClient({
    queryCache: new QueryCache({
      onError: (error, query) => {
        if (query.queryKey[0] !== CURRENT_SESSION) recheckSession(error);
      },
    }),
    mutationCache: new MutationCache({ onError: recheckSession }),
    // The engine runs in this process, so the web view's online state says nothing about it:
    // commands never wait for a network, and a reconnect never re-runs reads (audited ones
    // load only on an explicit open).
    defaultOptions: {
      queries: {
        retry: false,
        refetchOnWindowFocus: false,
        refetchOnReconnect: false,
        networkMode: "always",
      },
      mutations: { retry: false, networkMode: "always" },
    },
  });
  return client;
}

/**
 * Runs an engine query. The query key starts with the command name. With `enabled: false` it
 * does not run, and stays pending: for a read the session may not make (a refused read is
 * audited), or one that waits for a choice.
 * `keepPrevious` keeps a directory's last page visible during a new query. Callers must
 * label it as updating and disable pagination while fetching. Never use it for record views.
 */
export function useCommandQuery<T>(
  command: CommandName,
  args?: InvokeArgs,
  {
    refetchInterval,
    enabled,
    keepPrevious = false,
  }: { refetchInterval?: number; enabled?: boolean; keepPrevious?: boolean } = {},
) {
  return useQuery({
    queryKey: args === undefined ? [command] : [command, args],
    queryFn: () => call<T>(command, args),
    refetchInterval,
    enabled,
    placeholderData: keepPrevious ? keepPreviousData : undefined,
  });
}

/**
 * Whether the signed-in session holds `capability`, to offer an action only to people who
 * may take it. Presentation only: the engine checks the capability on every command.
 */
export function useHasCapability(capability: string): boolean {
  const session = useCommandQuery<SessionInfo | null>(CURRENT_SESSION);
  return session.data?.capabilities.includes(capability) ?? false;
}

interface PagesOptions<Item, Cursor> {
  /** What the pages are filtered by; part of the query key. */
  key: InvokeArgs;
  /** The arguments for the page after `cursor`; `undefined` for the first page. */
  pageArgs: (cursor: Cursor | undefined) => InvokeArgs;
  /** The cursor of the page after `page`, or `undefined` when `page` is the last one. */
  nextCursor: (page: Item[]) => Cursor | undefined;
  /** Background jobs alone may poll until all loaded jobs finish. */
  refetchInterval?: (pages: Item[][] | undefined) => number | false;
}

/** Runs an engine query page by page, loading the next page only when asked. */
export function useCommandPages<Item, Cursor = number>(
  command: CommandName,
  { key, pageArgs, nextCursor, refetchInterval }: PagesOptions<Item, Cursor>,
) {
  return useInfiniteQuery<Item[], CommandError, InfiniteData<Item[]>, QueryKey, Cursor | undefined>(
    {
      queryKey: [command, key],
      queryFn: ({ pageParam }) => call<Item[]>(command, pageArgs(pageParam as Cursor | undefined)),
      initialPageParam: undefined,
      getNextPageParam: nextCursor,
      refetchInterval: refetchInterval
        ? (query) =>
            query.state.status === "error" ? false : refetchInterval(query.state.data?.pages)
        : undefined,
    },
  );
}

/**
 * The queries a mutation makes stale, by command name. `"all"` clears every cached
 * result, for session changes: nothing from one person's session is shown to the next
 * (REQ-IDN-004).
 */
type Invalidates = readonly CommandName[] | "all";

/** The engine mirrors this allowlist with a typed action enum and original authorization. */
const KEY_CONFIRMABLE = new Set<string>([
  "create_user",
  "set_user_roles",
  "reset_user_credentials",
  "designate_one_person_laboratory",
  "set_separation_policy",
  "set_security_policy",
  "release_report",
  "withdraw_report",
  "export_recovery_kit",
  "request_exit_export",
  "close_recovery_reconciliation",
  "record_update_approval",
  "approve_downloaded_update",
  "accept_installed_version",
  "approve_reviewed_catalog",
  "revoke_security_key",
  "set_unlock_pin",
  "disable_unlock_pin",
]);
const KEY_OPERATIONS = new Set<string>([
  "security_key_signup",
  "security_key_sign_in",
  "security_key_unlock",
  "security_key_enroll",
  "security_key_recover",
  "security_key_action",
]);

function requiresKeyConfirmation(command: CommandName, input: unknown): boolean {
  if (!KEY_CONFIRMABLE.has(command)) return false;
  if (command !== "set_security_policy") return true;
  // Numeric policy edits need no fresh proof; changing key policy does.
  return (
    typeof input === "object" && input !== null && "password" in input && input.password != null
  );
}

/** Dispatches a sensitive key action as one immutable request; no proof returns to the UI. */
async function mutateCommand<T>(
  queryClient: QueryClient,
  command: CommandName,
  args: InvokeArgs,
): Promise<T> {
  const commandArgs = args as Record<string, unknown>;
  try {
    if (requiresKeyConfirmation(command, commandArgs.input)) {
      const status = await call<AuthenticationStatus>("get_authentication_status", {});
      if (status.method === "security_key") {
        const session = queryClient.getQueryData<SessionInfo | null>([CURRENT_SESSION]);
        const selected = session
          ? queryClient.getQueryData<string>(securityKeySelection(session.userId))
          : undefined;
        const credentialId =
          status.credentials.find((key) => key.credentialId === selected)?.credentialId ??
          status.credentials[0]?.credentialId;
        return await call<T>("security_key_action", {
          input: { credentialId, action: { command, input: commandArgs.input } },
        });
      }
    }
    return await call<T>(command, args);
  } finally {
    // React Query retains submitted variables. PINs and used recovery codes must not remain there.
    if (typeof commandArgs.input === "object" && commandArgs.input !== null) {
      const input = commandArgs.input as Record<string, unknown>;
      if ("pin" in input) input.pin = "";
      if ("recoveryCode" in input) input.recoveryCode = "";
    }
  }
}

/** Runs an engine command and refreshes the queries it declares. */
export function useCommandMutation<TResult, TArgs extends InvokeArgs = Record<string, never>>(
  command: CommandName,
  { invalidates }: { invalidates: Invalidates },
) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationKey:
      KEY_OPERATIONS.has(command) || KEY_CONFIRMABLE.has(command)
        ? ["security-key", command]
        : [command],
    gcTime: 0,
    mutationFn: (args: TArgs) => mutateCommand<TResult>(queryClient, command, args),
    onSuccess: () =>
      invalidates === "all"
        ? queryClient.resetQueries()
        : Promise.all(
            invalidates.map((name) => queryClient.invalidateQueries({ queryKey: [name] })),
          ),
  });
}
