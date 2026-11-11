// Interfaces (ticket #4555; REQ-INT-001–003, REQ-RPT-003, REQ-RPT-011–012, REQ-BIL-002):
// connections to outside systems, the message log and Unrouted messages. Every command needs
// `manage_configuration`. A connection's folder comes only from the native dialog; the UI holds
// the handle `choose_partner_folder` returns, never a path it sends.
import type { CommandName } from "@/generated/CommandName";

/** `list_partner_connections() → PartnerConnection[]`. */
export const LIST_CONNECTIONS = "list_partner_connections" satisfies CommandName;
/** `get_partner_connection({input:{connectionId}}) → PartnerConnectionDetail`. */
export const GET_CONNECTION = "get_partner_connection" satisfies CommandName;
/** `choose_partner_folder() → ChosenPartnerFolder | null` (opens the native dialog). */
export const CHOOSE_FOLDER = "choose_partner_folder" satisfies CommandName;
/** `test_partner_folder({input:{folderChoice}|{connectionId}}) → FolderTestResult`. */
export const TEST_FOLDER = "test_partner_folder" satisfies CommandName;
/** `test_partner_sftp({input:TestPartnerSftpInput}) → FolderTestResult` (saves nothing). */
export const TEST_SFTP = "test_partner_sftp" satisfies CommandName;
/** `test_partner_mllp({input:TestPartnerMllpInput}) → FolderTestResult` (connects, sends nothing). */
export const TEST_MLLP = "test_partner_mllp" satisfies CommandName;
/** `get_lis_sftp_key() → string`: the LIS's public SSH key, made on first use. */
export const GET_SFTP_KEY = "get_lis_sftp_key" satisfies CommandName;
export const CREATE_CONNECTION = "create_partner_connection" satisfies CommandName;
export const UPDATE_CONNECTION = "update_partner_connection" satisfies CommandName;
export const SET_ACTIVE = "set_partner_connection_active" satisfies CommandName;
export const RECORD_CHECK = "record_partner_destination_check" satisfies CommandName;
export const SEND_TEST_MESSAGE = "send_partner_test_message" satisfies CommandName;
/** `list_partner_messages({input:ListPartnerMessages}) → PartnerMessagePage`. */
export const LIST_MESSAGES = "list_partner_messages" satisfies CommandName;
/**
 * `get_partner_message({input:{deliveryId}}) → PartnerMessageDetail`: the HL7 message as sent,
 * its control ID and its attempts. Audited: it holds patient data, so load it only on open.
 */
export const GET_MESSAGE = "get_partner_message" satisfies CommandName;
export const EXPORT_MESSAGES = "export_partner_messages" satisfies CommandName;
/** `list_unrouted_partner_messages() → UnroutedMessage[]`. */
export const LIST_UNROUTED = "list_unrouted_partner_messages" satisfies CommandName;
export const RESOLVE_UNROUTED = "resolve_unrouted_message" satisfies CommandName;
export const RESEND_DELIVERY = "resend_partner_delivery" satisfies CommandName;
/** `reject_received_message({input:RejectReceivedMessage})`: a waiting or held order message. */
export const REJECT_RECEIVED = "reject_received_message" satisfies CommandName;
/** `resolve_partner_rejection({input:ResolvePartnerRejection})`: it leaves Rejected. */
export const RESOLVE_REJECTION = "resolve_partner_rejection" satisfies CommandName;
/** `list_partner_mappings({input:{connectionId}}) → PartnerMappings` (HL7 connections). */
export const LIST_MAPPINGS = "list_partner_mappings" satisfies CommandName;
/** Each `→ PartnerMappings`, the list after the change. */
export const DRAFT_MAPPING = "draft_partner_mapping" satisfies CommandName;
export const DISCARD_MAPPING_DRAFT = "discard_partner_mapping_draft" satisfies CommandName;
export const APPROVE_MAPPINGS = "approve_partner_mappings" satisfies CommandName;
/** Each `→ PartnerConnectionDetail`: the connection with its contract after the change. */
export const DRAFT_CONTRACT = "draft_partner_contract" satisfies CommandName;
export const APPROVE_CONTRACT = "approve_partner_contract" satisfies CommandName;
/** Runs the synthetic acceptance tests; sends nothing and keeps nothing they make. */
export const RUN_ACCEPTANCE = "run_partner_acceptance_tests" satisfies CommandName;

/** `list_clients() → ClientView[]`: an incoming connection's partner is one of them. */
export const LIST_CLIENTS = "list_clients" satisfies CommandName;

const WORK_QUEUES = "list_work_queues" satisfies CommandName;

/** What a code mapping change makes stale: approval changes the connection's history. */
export const MAPPING_QUERIES: readonly CommandName[] = [
  LIST_MAPPINGS,
  GET_CONNECTION,
  LIST_MESSAGES,
];

/** What a connection change makes stale. */
export const CONNECTION_QUERIES: readonly CommandName[] = [
  LIST_CONNECTIONS,
  GET_CONNECTION,
  LIST_MESSAGES,
];
/** What a message decision makes stale, the navigation badge included. */
export const MESSAGE_QUERIES: readonly CommandName[] = [
  LIST_CONNECTIONS,
  GET_CONNECTION,
  LIST_MESSAGES,
  GET_MESSAGE,
  LIST_UNROUTED,
  WORK_QUEUES,
];
