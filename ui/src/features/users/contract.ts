// The user administration contract (REQ-IDN-004). Accounts, non-clinical roles and resets need
// manage_users; clinical roles need manage_quality, and `list_roles` marks what the caller may
// assign. Creating users, changing roles and resetting sign-in carry the actor's `password`; own
// roles and sign-in are refused (`forbidden`). Changes to an existing user carry its `revision`
// and fail with `conflict` when stale.
// Types are generated from the engine into `@/generated`. Command names: list_roles,
// list_users, create_user, set_user_roles, disable_user, enable_user, reset_user_credentials.

/** `list_users() → UserSummary[]`; every user change refreshes it. */
export const LIST_USERS = "list_users";
/**
 * `designate_one_person_laboratory({input}) → UserSummary` (`manage_users`, password, reason;
 * ADR 0015): the only active account gets every role, and one person may do each duty alone.
 */
export const DESIGNATE_ONE_PERSON_LABORATORY = "designate_one_person_laboratory";
