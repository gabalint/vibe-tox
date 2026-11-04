//! Interfaces (ticket #4555): billing handoffs routed to Billing JSON connections, written
//! to their folder only after the destination check, logged without "delivered", and the
//! unrouted and failed paths that wait for a person (REQ-RPT-003, REQ-RPT-011,
//! REQ-RPT-012, REQ-INT-002, REQ-INT-003, REQ-BIL-002). Synthetic data only.

use std::fs;
use std::path::Path;

use lis_engine::{
    CreatePartnerConnectionInput, ListPartnerMessages, PartnerConnectionDetail,
    PartnerConnectionInput, RecordPartnerDestinationCheck, ResendPartnerDelivery,
    ResolveUnroutedMessage, SetPartnerConnectionActive, TestPartnerFolderInput,
    UpdatePartnerConnectionInput,
};
use lis_interfaces::partner::{
    ApprovePartnerMappings, DiscardPartnerMappingDraft, DraftPartnerMapping, MappingKind,
    MessageView, PartnerConnectionId, PartnerConnectionSettings, PartnerDeliveryId,
    PartnerMessageId, PartnerMessageRow, PartnerMessageType, PartnerProtocol, RoutingRule,
    RuleCondition, RuleField, Schedule, TransportOutcome, UnroutedDecision, WaitReason,
};
use tempfile::TempDir;

use super::paging::release_groups;
use super::*;

const PATTERN: &str = "billing-{handoff}.json";

pub(super) async fn partner_call(engine: &Engine, name: &str) -> Option<Result<(), CommandError>> {
    let connection_id = PartnerConnectionId::generate();
    Some(match name {
        "list_partner_connections" => engine.list_partner_connections().await.map(drop),
        "get_partner_connection" => engine
            .get_partner_connection(PartnerConnectionInput { connection_id })
            .await
            .map(drop),
        "choose_partner_folder" => engine.choose_partner_folder(None).await.map(drop),
        "test_partner_folder" => engine
            .test_partner_folder(TestPartnerFolderInput {
                folder_choice: None,
                connection_id: Some(connection_id),
            })
            .await
            .map(drop),
        "create_partner_connection" => engine
            .create_partner_connection(CreatePartnerConnectionInput {
                settings: settings("Synthetic", Vec::new()),
                folder_choice: lis_core::Id::generate(),
                active: true,
            })
            .await
            .map(drop),
        "update_partner_connection" => engine
            .update_partner_connection(UpdatePartnerConnectionInput {
                connection_id,
                expected_revision: 1,
                settings: settings("Synthetic", Vec::new()),
                folder_choice: None,
                reason: "Synthetic authorization test".to_owned(),
            })
            .await
            .map(drop),
        _ => return state_call(engine, name, connection_id).await,
    })
}

async fn state_call(
    engine: &Engine,
    name: &str,
    connection_id: PartnerConnectionId,
) -> Option<Result<(), CommandError>> {
    Some(match name {
        "set_partner_connection_active" => engine
            .set_partner_connection_active(SetPartnerConnectionActive {
                connection_id,
                expected_revision: 1,
                active: false,
                reason: "Synthetic authorization test".to_owned(),
            })
            .await
            .map(drop),
        "record_partner_destination_check" => engine
            .record_partner_destination_check(check(connection_id, 1))
            .await
            .map(drop),
        "send_partner_test_message" => engine
            .send_partner_test_message(PartnerConnectionInput { connection_id })
            .await
            .map(drop),
        "list_partner_messages" => engine.list_partner_messages(all()).await.map(drop),
        "export_partner_messages" => engine.export_partner_messages(all(), None).await.map(drop),
        "list_unrouted_partner_messages" => engine.list_unrouted_partner_messages().await.map(drop),
        "resolve_unrouted_message" => {
            engine
                .resolve_unrouted_message(ResolveUnroutedMessage {
                    message_id: PartnerMessageId::generate(),
                    decision: UnroutedDecision::RetryRouting,
                })
                .await
        }
        "resend_partner_delivery" => {
            engine
                .resend_partner_delivery(ResendPartnerDelivery {
                    delivery_id: PartnerDeliveryId::generate(),
                })
                .await
        }
        _ => return mapping_call(engine, name, connection_id).await,
    })
}

async fn mapping_call(
    engine: &Engine,
    name: &str,
    connection_id: PartnerConnectionId,
) -> Option<Result<(), CommandError>> {
    let lis_id = lis_core::Id::generate();
    Some(match name {
        "list_partner_mappings" => engine
            .list_partner_mappings(PartnerConnectionInput { connection_id })
            .await
            .map(drop),
        "draft_partner_mapping" => engine
            .draft_partner_mapping(DraftPartnerMapping {
                connection_id,
                kind: MappingKind::Analyte,
                lis_id,
                mapping: None,
            })
            .await
            .map(drop),
        "discard_partner_mapping_draft" => engine
            .discard_partner_mapping_draft(DiscardPartnerMappingDraft {
                connection_id,
                kind: MappingKind::Analyte,
                lis_id,
            })
            .await
            .map(drop),
        "approve_partner_mappings" => engine
            .approve_partner_mappings(ApprovePartnerMappings {
                connection_id,
                expected_revision: 1,
                reason: "Synthetic authorization test".to_owned(),
                self_approval: false,
            })
            .await
            .map(drop),
        _ => return None,
    })
}

fn settings(name: &str, rules: Vec<RoutingRule>) -> PartnerConnectionSettings {
    PartnerConnectionSettings {
        name: name.to_owned(),
        partner: "Synthetic billing company".to_owned(),
        message_type: PartnerMessageType::BillingJson,
        protocol: PartnerProtocol::Folder,
        rules,
        schedule: Schedule::Immediately,
        file_name_pattern: PATTERN.to_owned(),
    }
}

fn client_is(code: &str) -> Vec<RoutingRule> {
    vec![RoutingRule {
        field: RuleField::Client,
        condition: RuleCondition::Equals,
        value: code.to_owned(),
    }]
}

fn check(connection_id: PartnerConnectionId, revision: i64) -> RecordPartnerDestinationCheck {
    RecordPartnerDestinationCheck {
        connection_id,
        expected_revision: revision,
        method: "Synthetic call-back to the billing company".to_owned(),
        self_approval: false,
    }
}

fn all() -> ListPartnerMessages {
    ListPartnerMessages {
        since: None,
        connection_id: None,
        view: MessageView::All,
        search: None,
    }
}

fn files(folder: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(folder)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

/// Two approved derivations ready to hand off, a second administrator for the
/// destination check, and the connection's folder.
struct Partners {
    desk: Desk,
    second_admin: String,
    folder: TempDir,
    approved: Vec<DerivationView>,
}

/// Two specimens finally released, collected on dates apart from the journey's samples, so
/// each date of service can be handed off on its own.
async fn two_released_dates() -> Desk {
    let mut desk = Desk::start().await;
    let first = desk
        .add_sample(desk.lab.orderables.panel.1, late_evening(), true)
        .await;
    let earlier = CollectionTimeInput::Date {
        date: "2026-10-10".to_owned(),
    };
    let second = desk
        .add_sample(desk.lab.orderables.panel.1, earlier, true)
        .await;
    desk.lab.import(JOURNEY).await;
    for index in [first, second] {
        desk.import_drugs(index).await;
    }
    desk.lab.accept_qc().await;
    for index in [first, second] {
        desk.lab.approve_all(index).await.unwrap();
        desk.condition_not_met(index).await;
    }
    release_groups(&desk, [first, second]).await;
    desk
}

impl Partners {
    async fn start() -> Self {
        let desk = two_released_dates().await;
        desk.resync().await;
        desk.confirm_mapping().await;
        let mut approved = Vec::new();
        for date in ["2026-10-11", "2026-10-10"] {
            let derived = desk.derive(Some(date)).await;
            approved.push(desk.approve(&derived).await.unwrap());
        }
        desk.resync().await;
        desk.lab.sign_in(Role::Admin).await;
        desk.lab
            .t()
            .create_user("adm2", &[RoleCode::Administrator])
            .await;
        let second_admin = desk.lab.t().first_sign_in("adm2").await.1;
        desk.resync().await;
        Self {
            desk,
            second_admin,
            folder: tempfile::tempdir().unwrap(),
            approved,
        }
    }

    async fn admin(&self) -> &Engine {
        self.desk.lab.sign_in(Role::Admin).await;
        self.desk.engine()
    }

    async fn second_admin(&self) -> &Engine {
        self.desk
            .lab
            .t()
            .sign_in_with("adm2", NEW_PASSWORD, &self.second_admin)
            .await;
        self.desk.engine()
    }

    async fn connect(&self, rules: Vec<RoutingRule>) -> PartnerConnectionDetail {
        let engine = self.admin().await;
        let chosen = engine
            .choose_partner_folder(Some(self.folder.path().to_owned()))
            .await
            .unwrap()
            .unwrap();
        engine
            .create_partner_connection(CreatePartnerConnectionInput {
                settings: settings("Billing - Synthetic", rules),
                folder_choice: chosen.folder_choice,
                active: true,
            })
            .await
            .unwrap()
    }

    async fn hand_off(&self, index: usize) -> HandoffView {
        let handoff = self.desk.hand_off(&[&self.approved[index]]).await.unwrap();
        self.desk.resync().await;
        handoff
    }

    async fn rows(&self) -> Vec<PartnerMessageRow> {
        let engine = self.admin().await;
        engine.list_partner_messages(all()).await.unwrap().rows
    }

    async fn queue_count(&self, queue: &str) -> Option<usize> {
        let engine = self.admin().await;
        engine
            .list_work_queues()
            .await
            .unwrap()
            .into_iter()
            .find(|summary| summary.queue == queue)
            .and_then(|summary| summary.count)
    }

    fn count(&self, sql: &str) -> i64 {
        self.desk.count(sql)
    }
}

#[tokio::test]
async fn a_routed_handoff_is_written_once_after_its_destination_check() {
    let partners = Partners::start().await;
    let connection = partners.connect(client_is("SYN-CLIENT")).await;
    let handoff = partners.hand_off(0).await;

    // REQ-RPT-012: nothing is written before the destination check.
    partners.desk.engine().deliver_partner_messages().await;
    assert!(files(partners.folder.path()).is_empty());
    let rows = partners.rows().await;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].transport, TransportOutcome::NotSent);
    assert_eq!(rows[0].waiting, Some(WaitReason::Unverified));
    assert_eq!(rows[0].source_number, Some(handoff.handoff_no));

    verify(&partners, &connection).await;
    partners.desk.engine().deliver_partner_messages().await;
    let name = format!("billing-{}.json", handoff.handoff_no);
    assert_eq!(files(partners.folder.path()), std::slice::from_ref(&name));
    let written: serde_json::Value =
        serde_json::from_slice(&fs::read(partners.folder.path().join(&name)).unwrap()).unwrap();
    assert_eq!(written["format"], "bil-handoff-v2");
    assert_eq!(written["handoffId"], handoff.handoff_id.to_string());

    // REQ-RPT-011: written, and the folder cannot confirm more.
    let rows = partners.rows().await;
    assert_eq!(rows[0].transport, TransportOutcome::Written);
    assert!(rows[0].outcome_unknown);
    // A second pass writes nothing more (REQ-INT-003).
    partners.desk.engine().deliver_partner_messages().await;
    assert_eq!(files(partners.folder.path()).len(), 1);
    assert_eq!(
        partners.count(
            "SELECT count(*) FROM audit_events \
             WHERE action = 'interfaces.write_message' AND outcome = 'succeeded'"
        ),
        1
    );
    unrouted_then_failed_then_resent(&partners, &connection).await;
}

/// The creator cannot check their own folder under the default policy; a second
/// administrator can.
async fn verify(partners: &Partners, connection: &PartnerConnectionDetail) {
    let id = connection.connection.id;
    let revision = connection.connection.revision;
    let refused = partners
        .admin()
        .await
        .record_partner_destination_check(check(id, revision))
        .await
        .unwrap_err();
    assert_eq!(refused.code(), ErrorCode::GateBlocked);
    assert_eq!(gate_codes(&refused), ["second_person_required"]);
    let verified = partners
        .second_admin()
        .await
        .record_partner_destination_check(check(id, revision))
        .await
        .unwrap();
    partners.desk.resync().await;
    assert!(verified.connection.verification.is_some());
    assert_eq!(verified.connection.revision, revision + 1);
}

/// A handoff no rule matches waits in Unrouted messages; sent once to a folder that is
/// gone, it fails five times, waits in the failures queue, and is written when sent again.
async fn unrouted_then_failed_then_resent(
    partners: &Partners,
    connection: &PartnerConnectionDetail,
) {
    only_another_client(partners, connection.connection.id).await;
    let handoff = partners.hand_off(1).await;
    let unrouted = partners
        .admin()
        .await
        .list_unrouted_partner_messages()
        .await
        .unwrap();
    assert_eq!(unrouted.len(), 1);
    assert_eq!(unrouted[0].source_number, Some(handoff.handoff_no));
    let closest = unrouted[0].closest.as_ref().unwrap();
    assert_eq!(closest.rule.value, "SYN-OTHER");
    assert_eq!(partners.queue_count("unrouted_messages").await, Some(1));
    let message_id = unrouted[0].message_id;
    let retry = partners
        .admin()
        .await
        .resolve_unrouted_message(ResolveUnroutedMessage {
            message_id,
            decision: UnroutedDecision::RetryRouting,
        })
        .await
        .unwrap_err();
    assert_eq!(retry.code(), ErrorCode::Conflict);
    fail_five_times(partners, message_id, connection.connection.id).await;
    resend_and_write(partners, handoff.handoff_no).await;
}

/// Changes the connection's rule so the next handoff matches no connection.
async fn only_another_client(partners: &Partners, connection_id: PartnerConnectionId) {
    let engine = partners.admin().await;
    let current = engine
        .get_partner_connection(PartnerConnectionInput { connection_id })
        .await
        .unwrap();
    let changed = engine
        .update_partner_connection(UpdatePartnerConnectionInput {
            connection_id,
            expected_revision: current.connection.revision,
            settings: settings("Billing - Synthetic", client_is("SYN-OTHER")),
            folder_choice: None,
            reason: "Synthetic: only another client".to_owned(),
        })
        .await
        .unwrap();
    // The folder did not change, so its destination check still holds.
    assert!(changed.connection.verification.is_some());
    assert_eq!(changed.history[0].fields, ["rules"]);
}

async fn fail_five_times(
    partners: &Partners,
    message_id: PartnerMessageId,
    connection: PartnerConnectionId,
) {
    let moved = partners.folder.path().with_extension("away");
    fs::rename(partners.folder.path(), &moved).unwrap();
    partners
        .admin()
        .await
        .resolve_unrouted_message(ResolveUnroutedMessage {
            message_id,
            decision: UnroutedDecision::SendOnce { connection },
        })
        .await
        .unwrap();
    for minutes in [0, 1, 2, 4, 8] {
        partners.desk.lab.t().advance(minutes * 60);
        partners.desk.engine().deliver_partner_messages().await;
    }
    // The passes ran with nobody at the keyboard; the idle session locked meanwhile.
    partners.desk.resync().await;
    let failed = partners.rows().await;
    let row = failed
        .iter()
        .find(|row| row.transport == TransportOutcome::Failed)
        .unwrap();
    assert!(row.given_up);
    assert_eq!(row.attempts, 5);
    assert_eq!(row.error_code.as_deref(), Some("folder_missing"));
    assert_eq!(partners.queue_count("interface_failures").await, Some(1));
    fs::rename(&moved, partners.folder.path()).unwrap();
}

async fn resend_and_write(partners: &Partners, handoff_no: i64) {
    let delivery_id = partners
        .rows()
        .await
        .into_iter()
        .find(|row| row.given_up)
        .and_then(|row| row.delivery_id)
        .unwrap();
    partners
        .admin()
        .await
        .resend_partner_delivery(ResendPartnerDelivery { delivery_id })
        .await
        .unwrap();
    partners.desk.engine().deliver_partner_messages().await;
    assert!(files(partners.folder.path()).contains(&format!("billing-{handoff_no}.json")));
    assert_eq!(partners.queue_count("interface_failures").await, Some(0));
    assert_eq!(partners.queue_count("unrouted_messages").await, Some(0));
}
