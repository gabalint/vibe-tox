// The front-desk commands shared by patients, orders and specimens (REQ-IDN-001). Types are
// generated from the engine into `@/generated`; order and accession views carry the patient
// by ID, so screens load the patient with `get_subject` to show who the record belongs to.

/**
 * `get_subject({input:{subjectId}}) → SubjectView` (audited read, `enter_orders` or `manage_quality`): every
 * assertion, identifier and open identity hold.
 */
export const GET_SUBJECT = "get_subject";
