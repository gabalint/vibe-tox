//! Subject registration, demographic assertions and identifiers (REQ-IDN-001).

use lis_audit::{AuditDetail, AuditEvent, Outcome, Stream};
use lis_core::Capability;
use lis_store::CommandContext;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::{IdentityError, ensure_revision};
use crate::model::{
    ActorRef, Demographics, DemographicsInput, Identifier, IdentifierInput, IdentityAssertionId,
    IdentityHoldId, NAME_MAX, REASON_MAX, SubjectId, SubjectIdentifierId, parse_identifiers,
    required_text,
};
use crate::repo::holds;
use crate::repo::subjects::{self as repo, NewAssertion};
use crate::views::HoldKind;

/// Input of [`register_subject`].
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[ts(export)]
pub struct RegisterSubject {
    /// Demographics as provided.
    pub demographics: DemographicsInput,
    /// Where the demographics came from, such as `requisition`.
    pub source: String,
    /// Identifiers as provided, each scoped by authority and type.
    #[serde(default)]
    pub identifiers: Vec<IdentifierInput>,
    /// The name and birth date candidates the user reviewed; must equal the current set.
    #[serde(default)]
    pub acknowledged_candidates: Vec<SubjectId>,
}

/// Result of [`register_subject`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SubjectRegistered {
    /// The new subject.
    pub subject_id: SubjectId,
    /// Its revision (1).
    #[ts(type = "number")]
    pub revision: i64,
    /// The possible-duplicate hold opened because candidates existed.
    pub identity_hold_id: Option<IdentityHoldId>,
}

/// Registers a subject with its first assertion and identifiers (REQ-IDN-001).
///
/// An active identifier already held within the same authority and type
/// names the existing subject, so registration is refused with
/// [`IdentityError::IdentifierInUse`]. Subjects with the same normalized name
/// and birth date are only candidates: the request must acknowledge exactly
/// that set, and the new subject then gets an open possible-duplicate hold.
pub fn register_subject(
    ctx: &mut CommandContext<'_>,
    input: RegisterSubject,
) -> Result<SubjectRegistered, IdentityError> {
    ctx.require(Capability::EnterOrders)?;
    let demographics = Demographics::parse(&input.demographics)?;
    let source = required_text("source", &input.source, NAME_MAX)?;
    let identifiers = parse_identifiers(&input.identifiers)?;
    let subject_id = SubjectId::generate();
    for identifier in &identifiers {
        ensure_identifier_available(ctx, subject_id, identifier)?;
    }
    // REQ-IDN-001: name and birth date matches are listed for review, never linked.
    let candidates = repo::subjects_by_match_key(ctx.tx(), &demographics.match_key(), None)?;
    if !same_set(&candidates, &input.acknowledged_candidates) {
        return Err(IdentityError::CandidatesNotAcknowledged);
    }
    repo::insert_subject(ctx.tx(), subject_id, ctx.now())?;
    let actor = ActorRef::of(ctx.principal());
    add_assertion(ctx, subject_id, &demographics, &source, None, &actor)?;
    for identifier in &identifiers {
        repo::insert_identifier(
            ctx.tx(),
            SubjectIdentifierId::generate(),
            subject_id,
            identifier,
            ctx.now(),
        )?;
    }
    ctx.audit(
        AuditEvent::new(
            Stream::Clinical,
            "identity.register_subject",
            Outcome::Succeeded,
        )
        .record("subject", subject_id)
        .new_revision(1)
        .detail("identifier_count", AuditDetail::count(identifiers.len())),
    )?;
    let identity_hold_id = if candidates.is_empty() {
        None
    } else {
        Some(open_duplicate_hold(ctx, subject_id, 1, &candidates)?)
    };
    Ok(SubjectRegistered {
        subject_id,
        revision: 1,
        identity_hold_id,
    })
}

/// Input of [`assert_demographics`].
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[ts(export)]
pub struct AssertDemographics {
    /// The subject.
    pub subject_id: SubjectId,
    /// The subject revision the user saw.
    #[ts(type = "number")]
    pub expected_revision: i64,
    /// The new demographics as provided.
    pub demographics: DemographicsInput,
    /// Where they came from.
    pub source: String,
    /// Why they changed.
    pub reason: String,
    /// The name and birth date candidates the user reviewed; must equal the current set.
    #[serde(default)]
    pub acknowledged_candidates: Vec<SubjectId>,
}

/// A subject's revision after a change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SubjectRevision {
    /// The subject.
    pub subject_id: SubjectId,
    /// Its new revision.
    #[ts(type = "number")]
    pub revision: i64,
}

/// Adds a demographic assertion with a reason; earlier assertions stay unchanged.
pub fn assert_demographics(
    ctx: &mut CommandContext<'_>,
    input: AssertDemographics,
) -> Result<SubjectRevision, IdentityError> {
    ctx.require(Capability::ManageQuality)?;
    let actor = ActorRef::user(ctx.principal())?;
    let demographics = Demographics::parse(&input.demographics)?;
    let source = required_text("source", &input.source, NAME_MAX)?;
    let reason = required_text("reason", &input.reason, REASON_MAX)?;
    let revision = checked_subject_revision(ctx, input.subject_id, input.expected_revision)?;
    let candidates =
        repo::subjects_by_match_key(ctx.tx(), &demographics.match_key(), Some(input.subject_id))?;
    if !same_set(&candidates, &input.acknowledged_candidates) {
        return Err(IdentityError::CandidatesNotAcknowledged);
    }
    let sequence = add_assertion(
        ctx,
        input.subject_id,
        &demographics,
        &source,
        Some(&reason),
        &actor,
    )?;
    repo::set_subject_revision(ctx.tx(), input.subject_id, revision + 1)?;
    ctx.audit(
        AuditEvent::new(
            Stream::Clinical,
            "identity.assert_demographics",
            Outcome::Succeeded,
        )
        .record("subject", input.subject_id)
        .old_revision(revision)
        .new_revision(revision + 1)
        .reason(reason)
        .detail("assertion_sequence", AuditDetail::integer(sequence)),
    )?;
    if !candidates.is_empty() {
        open_duplicate_hold(ctx, input.subject_id, revision + 1, &candidates)?;
    }
    Ok(SubjectRevision {
        subject_id: input.subject_id,
        revision: revision + 1,
    })
}

/// Input of [`add_subject_identifier`].
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[ts(export)]
pub struct AddSubjectIdentifier {
    /// The subject.
    pub subject_id: SubjectId,
    /// The subject revision the user saw.
    #[ts(type = "number")]
    pub expected_revision: i64,
    /// The identifier as provided.
    pub identifier: IdentifierInput,
}

/// Result of [`add_subject_identifier`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct IdentifierAdded {
    /// The new identifier row.
    pub identifier_id: SubjectIdentifierId,
    /// The subject's new revision.
    #[ts(type = "number")]
    pub subject_revision: i64,
}

/// Adds an identifier; historical ownership by another subject is a conflict (INV-02).
pub fn add_subject_identifier(
    ctx: &mut CommandContext<'_>,
    input: AddSubjectIdentifier,
) -> Result<IdentifierAdded, IdentityError> {
    ctx.require(Capability::EnterOrders)?;
    let identifier = Identifier::parse("identifier", &input.identifier)?;
    let revision = checked_subject_revision(ctx, input.subject_id, input.expected_revision)?;
    ensure_identifier_available(ctx, input.subject_id, &identifier)?;
    let identifier_id = SubjectIdentifierId::generate();
    repo::insert_identifier(
        ctx.tx(),
        identifier_id,
        input.subject_id,
        &identifier,
        ctx.now(),
    )?;
    repo::set_subject_revision(ctx.tx(), input.subject_id, revision + 1)?;
    ctx.audit(
        AuditEvent::new(
            Stream::Clinical,
            "identity.add_identifier",
            Outcome::Succeeded,
        )
        .record("subject", input.subject_id)
        .old_revision(revision)
        .new_revision(revision + 1)
        .detail("identifier_id", AuditDetail::id(identifier_id)),
    )?;
    Ok(IdentifierAdded {
        identifier_id,
        subject_revision: revision + 1,
    })
}

/// Input of [`deactivate_subject_identifier`].
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[ts(export)]
pub struct DeactivateSubjectIdentifier {
    /// The identifier row.
    pub identifier_id: SubjectIdentifierId,
    /// The owning subject's revision the user saw.
    #[ts(type = "number")]
    pub expected_subject_revision: i64,
    /// Why, such as a mistyped value.
    pub reason: String,
}

/// Deactivates an identifier with a reason; the row and its value are kept.
pub fn deactivate_subject_identifier(
    ctx: &mut CommandContext<'_>,
    input: DeactivateSubjectIdentifier,
) -> Result<SubjectRevision, IdentityError> {
    ctx.require(Capability::ManageQuality)?;
    ActorRef::user(ctx.principal())?;
    let reason = required_text("reason", &input.reason, REASON_MAX)?;
    let (subject_id, active) = repo::identifier_state(ctx.tx(), input.identifier_id)?
        .ok_or(IdentityError::NotFound("identifier"))?;
    let revision = checked_subject_revision(ctx, subject_id, input.expected_subject_revision)?;
    if !active {
        return Err(IdentityError::IdentifierInactive);
    }
    repo::deactivate_identifier(ctx.tx(), input.identifier_id, ctx.now(), &reason)?;
    repo::set_subject_revision(ctx.tx(), subject_id, revision + 1)?;
    ctx.audit(
        AuditEvent::new(
            Stream::Clinical,
            "identity.deactivate_identifier",
            Outcome::Succeeded,
        )
        .record("subject", subject_id)
        .old_revision(revision)
        .new_revision(revision + 1)
        .reason(reason)
        .detail("identifier_id", AuditDetail::id(input.identifier_id)),
    )?;
    Ok(SubjectRevision {
        subject_id,
        revision: revision + 1,
    })
}

fn ensure_identifier_available(
    ctx: &CommandContext<'_>,
    subject_id: SubjectId,
    identifier: &Identifier,
) -> Result<(), IdentityError> {
    if let Some(owner) = repo::active_identifier_owner(ctx.tx(), identifier)? {
        return Err(IdentityError::IdentifierInUse(owner));
    }
    match repo::historical_identifier_owner(ctx.tx(), identifier)? {
        Some(owner) if owner != subject_id => Err(IdentityError::IdentifierInUse(owner)),
        Some(_) | None => Ok(()),
    }
}

/// The subject's current revision, after refusing a missing subject or a stale revision.
fn checked_subject_revision(
    ctx: &CommandContext<'_>,
    subject_id: SubjectId,
    expected: i64,
) -> Result<i64, IdentityError> {
    let revision =
        repo::subject_revision(ctx.tx(), subject_id)?.ok_or(IdentityError::NotFound("subject"))?;
    ensure_revision(revision, expected)?;
    Ok(revision)
}

/// Appends the next assertion and returns its sequence number.
fn add_assertion(
    ctx: &CommandContext<'_>,
    subject_id: SubjectId,
    demographics: &Demographics,
    source: &str,
    reason: Option<&str>,
    asserted_by: &ActorRef,
) -> Result<i64, IdentityError> {
    let sequence = repo::next_assertion_sequence(ctx.tx(), subject_id)?;
    repo::insert_assertion(
        ctx.tx(),
        &NewAssertion {
            id: IdentityAssertionId::generate(),
            subject_id,
            sequence,
            demographics,
            source,
            reason,
            asserted_at: ctx.now(),
            asserted_by,
        },
    )?;
    Ok(sequence)
}

fn open_duplicate_hold(
    ctx: &CommandContext<'_>,
    subject_id: SubjectId,
    subject_revision: i64,
    candidates: &[SubjectId],
) -> Result<IdentityHoldId, IdentityError> {
    let hold_id = IdentityHoldId::generate();
    holds::insert_hold(
        ctx.tx(),
        hold_id,
        subject_id,
        subject_revision,
        HoldKind::PossibleDuplicate,
        ctx.now(),
    )?;
    for candidate in candidates {
        holds::insert_candidate(ctx.tx(), hold_id, *candidate)?;
    }
    ctx.audit(
        AuditEvent::new(Stream::Clinical, "identity.open_hold", Outcome::Succeeded)
            .record("identity_hold", hold_id)
            .new_revision(1)
            .detail(
                "kind",
                AuditDetail::code(HoldKind::PossibleDuplicate.as_str()),
            )
            .detail("subject_id", AuditDetail::id(subject_id))
            .detail("candidate_count", AuditDetail::count(candidates.len())),
    )?;
    Ok(hold_id)
}

/// Order-insensitive equality of two ID lists, ignoring repeats.
fn same_set(current: &[SubjectId], acknowledged: &[SubjectId]) -> bool {
    let sorted = |ids: &[SubjectId]| {
        let mut ids = ids.to_vec();
        ids.sort_unstable();
        ids.dedup();
        ids
    };
    sorted(current) == sorted(acknowledged)
}
