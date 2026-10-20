// Patient search, registration and identity holds (REQ-IDN-001). Types are generated from
// the engine into `@/generated`. Search and matching permit `enter_orders` or `manage_quality`; registration needs
// `enter_orders`, while resolving a hold needs `manage_quality`.

/** `search_subjects({input: SubjectSearch}) → SubjectSummary[]` (audited read). */
export const SEARCH_SUBJECTS = "search_subjects";

/**
 * `match_subject_candidates({input: SubjectMatchQuery}) → SubjectCandidates` (audited
 * read): identifier matches are the same patient; name and birth date candidates are only
 * listed for review, never linked.
 */
export const MATCH_SUBJECT_CANDIDATES = "match_subject_candidates";

/**
 * `register_subject({input: RegisterSubject}) → SubjectRegistered`. Refused with `conflict`
 * when the candidates changed since they were shown, or an identifier is already held.
 */
export const REGISTER_SUBJECT = "register_subject";

/** `resolve_identity_hold({input: ConfirmDistinctSubjects}) → IdentityHoldResolved`. */
export const RESOLVE_IDENTITY_HOLD = "resolve_identity_hold";

/** The capability that resolves identity holds; others see the holds only. */
export const MANAGE_QUALITY = "manage_quality";
