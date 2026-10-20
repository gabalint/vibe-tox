//! WFREV-02: activation changes new orders, never accepted pinned work.

use lis_analytical::{AttemptKind, CreatePrepBatchInput, CreateRunInput, RevisionIndicator};
use lis_catalog::{
    ApproveReviewedCatalog, CatalogDraft, CatalogPreviewId, ComponentInput, DraftMethodVersion,
    DraftOrderableVersion, MethodVersionContent, MethodVersionId, OrderableVersionContent,
    OrderableVersionId, RecordCatalogPreview, RetireMethodVersion, VersionStatus,
};
use lis_core::DecimalText;

use crate::{Fixture, STAFF, URINE, fixture, gate_codes, patient_input, read, run};

/// Use the normal coordinated review to move both the method and orderable.
fn supersede(f: &Fixture) -> (MethodVersionId, OrderableVersionId) {
    let old_method = f.urine.method;
    let old_test = f.urine.test;
    let method = read(f, move |conn| {
        Ok(lis_catalog::get_method_version(conn, old_method)?)
    });
    let test = read(f, move |conn| {
        Ok(lis_catalog::get_orderable_version(conn, old_test)?)
    });
    let method = f.setup(move |ctx| {
        lis_catalog::draft_method_version(
            ctx,
            DraftMethodVersion {
                method_id: method.method_id,
                content: MethodVersionContent {
                    report_notes: None,
                    specimen_type_id: method.specimen_type_id,
                    certainty: method.certainty,
                    output_scale: method.output_scale,
                    threshold_rule: method.threshold_rule,
                    threshold_rounding: method.threshold_rounding,
                    analytes: method.analytes.into_iter().map(|a| a.limits).collect(),
                },
            },
        )
    });
    let test = f.setup(move |ctx| {
        lis_catalog::draft_orderable_version(
            ctx,
            DraftOrderableVersion {
                orderable_id: test.orderable_id,
                content: OrderableVersionContent {
                    components: test
                        .components
                        .into_iter()
                        .map(|c| ComponentInput {
                            method_version_id: method.id,
                            purpose: c.purpose,
                            analyte_ids: c.analytes.into_iter().map(|a| a.analyte_id).collect(),
                        })
                        .collect(),
                },
            },
        )
    });
    let preview_id = f.setup(move |ctx| {
        lis_catalog::record_catalog_preview(
            ctx,
            RecordCatalogPreview {
                preview_id: CatalogPreviewId::generate(),
                content_digest: "a".repeat(64),
                artifact_digest: "b".repeat(64),
                drafts: vec![
                    CatalogDraft::MethodVersion {
                        id: method.id,
                        revision: method.revision,
                    },
                    CatalogDraft::OrderableVersion {
                        id: test.id,
                        revision: test.revision,
                    },
                ],
            },
        )
    });
    f.setup(move |ctx| {
        lis_catalog::approve_reviewed_catalog(
            ctx,
            ApproveReviewedCatalog {
                external_approval_reference: None,
                preview_id,
                confirmed_blank_cutoffs: Vec::new(),
                self_approval: false,
            },
        )
    });
    (method.id, test.id)
}

fn assert_repeat_kinds(f: &Fixture, label: &str, initial: &lis_analytical::ExecutionView) {
    let original = f.urine.method;
    let prep = run(f, STAFF, move |ctx| {
        lis_analytical::create_prep_batch(
            ctx,
            CreatePrepBatchInput {
                method_version_id: original,
            },
        )
    })
    .unwrap();
    f.register_source(RevisionIndicator::None, true);
    for kind in [
        AttemptKind::Rerun,
        AttemptKind::Reextraction,
        AttemptKind::Dilution,
    ] {
        let repeat_run = f.open_run();
        let mut input = patient_input(repeat_run.run_id, "S1", label);
        input.attempt_kind = kind;
        input.related_execution_id = Some(initial.execution_id);
        input.prep_batch_id = Some(prep.prep_batch_id);
        input.reason = Some("Synthetic repeat after routine supersession".to_owned());
        if kind == AttemptKind::Dilution {
            input.dilution_factor = Some(DecimalText::parse("10.00").unwrap());
        }
        let repeated = run(f, STAFF, move |ctx| {
            lis_analytical::add_patient_position(ctx, input)
        })
        .unwrap();
        assert_eq!(repeated.method_version_id, original);
        let execution = repeated.positions[0].execution.as_ref().unwrap();
        assert_eq!(execution.component_ids, initial.component_ids);
        assert_eq!(execution.related_execution_id, Some(initial.execution_id));
        if kind == AttemptKind::Dilution {
            assert_eq!(
                execution.dilution_factor.as_ref().unwrap().as_str(),
                "10.00"
            );
        }
    }
}

#[test]
fn accepted_unstarted_work_and_all_repeat_kinds_keep_the_original_method() {
    let f = fixture();
    let old_material = f.urine_material();
    let original = f.urine.method;
    let (current, current_test) = supersede(&f);
    let history = read(&f, move |conn| {
        Ok(lis_catalog::get_method_version(conn, original)?)
    });
    assert_eq!(history.lifecycle.status, VersionStatus::Approved);
    assert_eq!(history.lifecycle.replaced_by, Some(current));
    let pending = read(&f, move |conn| {
        lis_analytical::list_worklist(conn, original)
    });
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].label_code, old_material.label);

    let first = f
        .place(f.open_run().run_id, "S1", &old_material.label)
        .unwrap();
    let initial = first.positions[0].execution.as_ref().unwrap();
    assert_repeat_kinds(&f, &old_material.label, initial);
    let new_material = f.material(current_test, URINE, true);
    let instrument_id = f.instrument;
    let new_run = run(&f, STAFF, move |ctx| {
        lis_analytical::create_run(
            ctx,
            CreateRunInput {
                instrument_id,
                method_version_id: current,
                lot_ids: Vec::new(),
            },
        )
    })
    .unwrap();
    assert!(matches!(
        f.place(new_run.run_id, "OLD", &old_material.label),
        Err(lis_analytical::AnalyticalError::NothingToTest)
    ));
    let placed = f.place(new_run.run_id, "NEW", &new_material.label).unwrap();
    assert_eq!(placed.method_version_id, current);
    let retained = read(&f, move |conn| {
        lis_analytical::get_run(conn, first.run_id, crate::test_start())
    });
    assert_eq!(retained.positions[0].execution.as_ref().unwrap(), initial);
}

#[test]
fn explicit_retirement_blocks_old_runs_preparation_and_placement() {
    let f = fixture();
    let material = f.urine_material();
    let open = f.open_run();
    supersede(&f);
    let original = f.urine.method;
    let version = read(&f, move |conn| {
        Ok(lis_catalog::get_method_version(conn, original)?)
    });
    f.setup(move |ctx| {
        lis_catalog::retire_method_version(
            ctx,
            RetireMethodVersion {
                method_version_id: original,
                expected_revision: version.lifecycle.revision,
                reason: "Synthetic safety suspension: do not continue testing".to_owned(),
            },
        )
    });
    assert_eq!(
        gate_codes(f.place(open.run_id, "S1", &material.label)),
        ["method_not_current"]
    );
    let instrument_id = f.instrument;
    let create = run(&f, STAFF, move |ctx| {
        lis_analytical::create_run(
            ctx,
            CreateRunInput {
                instrument_id,
                method_version_id: original,
                lot_ids: Vec::new(),
            },
        )
    });
    assert_eq!(gate_codes(create), ["method_not_current"]);
    let prepare = run(&f, STAFF, move |ctx| {
        lis_analytical::create_prep_batch(
            ctx,
            CreatePrepBatchInput {
                method_version_id: original,
            },
        )
    });
    assert_eq!(gate_codes(prepare), ["method_not_current"]);
}

fn profile(f: &Fixture, version: Option<i64>, name: &str) -> lis_identity::LaboratoryProfileId {
    let input = lis_identity::SetLaboratoryProfile {
        expected_version: version,
        name: name.to_owned(),
        address_lines: vec!["100 Synthetic Street".to_owned()],
        clia_id: "12D1234567".to_owned(),
        director: "Synthetic Director".to_owned(),
        phone: "555-0100".to_owned(),
        time_zone: "America/New_York".to_owned(),
        billing_npi: None,
        tax_id: None,
        reason: version.map(|_| "Synthetic site transition".to_owned()),
    };
    f.setup(move |ctx| lis_identity::set_laboratory_profile(ctx, input))
        .profile_id
}

#[test]
fn performing_site_stays_pinned_across_profile_changes_and_unknown_stays_unknown() {
    let f = fixture();
    let unknown = f.open_run().run_id;
    let old = profile(&f, None, "Synthetic original site");
    let material = f.urine_material();
    let first = f.place(f.open_run().run_id, "S1", &material.label).unwrap();
    let execution = first.positions[0].execution.as_ref().unwrap().execution_id;
    let current = profile(&f, Some(1), "Synthetic new site");
    let next = f.open_run().run_id;
    assert_eq!(
        read(&f, move |conn| lis_analytical::run_laboratory_profile(
            conn, unknown
        )),
        None
    );
    assert_eq!(
        read(
            &f,
            move |conn| lis_analytical::execution_laboratory_profile(conn, execution)
        ),
        Some(old)
    );
    assert_eq!(
        read(&f, move |conn| lis_analytical::run_laboratory_profile(
            conn, next
        )),
        Some(current)
    );
    assert!(
        f.raw()
            .execute(
                "UPDATE ana_runs SET laboratory_profile_id = ?1 WHERE id = ?2",
                rusqlite::params![current, first.run_id]
            )
            .is_err()
    );
}

#[test]
fn legacy_run_profile_migration_does_not_infer_a_site_from_wall_clock_time() {
    let f = fixture();
    profile(&f, None, "Synthetic configured site");
    let old = f.open_run().run_id;
    let raw = f.raw();
    raw.execute_batch(
        "DROP TABLE ana_run_performing_site_resolutions;
         DROP TRIGGER ana_runs_profile_immutable;
         ALTER TABLE ana_runs DROP COLUMN laboratory_profile_id;",
    )
    .unwrap();
    raw.execute_batch(lis_analytical::MIGRATIONS[6].sql)
        .unwrap();
    assert_eq!(
        read(&f, move |conn| lis_analytical::run_laboratory_profile(
            conn, old
        )),
        None
    );
    let violations: i64 = raw
        .query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(violations, 0);
}

#[test]
fn unknown_site_resolution_needs_quality_authority_current_revision_and_retained_evidence() {
    let f = fixture();
    let unknown = f.open_run();
    let site = profile(&f, None, "Synthetic evidenced site");
    let input = lis_analytical::RecordRunPerformingSiteInput {
        run_id: unknown.run_id,
        expected_revision: unknown.revision,
        laboratory_profile_id: site,
        source_reference: "SYN-BENCH-LOG-3".to_owned(),
        reason: "Original bench log identifies the performing site".to_owned(),
    };
    let denied = input.clone();
    assert_eq!(
        crate::code_of(run(
            &f,
            &[lis_core::Capability::PerformAnalysis],
            move |ctx| lis_analytical::record_run_performing_site(ctx, denied)
        )),
        lis_core::ErrorCode::Forbidden
    );
    let mut stale = input.clone();
    stale.expected_revision += 1;
    assert_eq!(
        crate::code_of(run(&f, STAFF, move |ctx| {
            lis_analytical::record_run_performing_site(ctx, stale)
        })),
        lis_core::ErrorCode::Conflict
    );
    let mut empty = input.clone();
    empty.source_reference.clear();
    assert_eq!(
        crate::code_of(run(&f, STAFF, move |ctx| {
            lis_analytical::record_run_performing_site(ctx, empty)
        })),
        lis_core::ErrorCode::Validation
    );
    let valid = input.clone();
    let resolved = run(&f, STAFF, move |ctx| {
        lis_analytical::record_run_performing_site(ctx, valid)
    })
    .unwrap();
    assert_eq!(resolved.laboratory_profile_id, Some(site));
    assert_eq!(
        resolved.performing_site_source.as_deref(),
        Some("SYN-BENCH-LOG-3")
    );
    assert_eq!(
        crate::last_event(&f).0,
        "analytical.record_run_performing_site"
    );
    assert_eq!(
        crate::code_of(run(&f, STAFF, move |ctx| {
            lis_analytical::record_run_performing_site(ctx, input)
        })),
        lis_core::ErrorCode::Conflict
    );
    assert!(
        f.raw()
            .execute("DELETE FROM ana_run_performing_site_resolutions", [])
            .is_err()
    );
    let known = f.open_run();
    let blocked = lis_analytical::RecordRunPerformingSiteInput {
        run_id: known.run_id,
        expected_revision: known.revision,
        laboratory_profile_id: site,
        source_reference: "SYN-OTHER".to_owned(),
        reason: "Cannot replace known pin".to_owned(),
    };
    assert_eq!(
        crate::code_of(run(&f, STAFF, move |ctx| {
            lis_analytical::record_run_performing_site(ctx, blocked)
        })),
        lis_core::ErrorCode::Conflict
    );
}

#[test]
fn preparation_pages_reach_every_batch_and_reopen_recorded_steps() {
    let f = fixture();
    let method = f.urine.method;
    let mut ids = Vec::new();
    for _ in 0..3 {
        let batch = run(&f, STAFF, move |ctx| {
            lis_analytical::create_prep_batch(
                ctx,
                CreatePrepBatchInput {
                    method_version_id: method,
                },
            )
        })
        .unwrap();
        ids.push(batch.prep_batch_id);
    }
    let batch = ids[0];
    run(&f, STAFF, move |ctx| {
        lis_analytical::record_prep_step(
            ctx,
            lis_analytical::RecordPrepStepInput {
                prep_batch_id: batch,
                step_code: "extract".to_owned(),
                lot_id: None,
                note: None,
            },
        )
    })
    .unwrap();
    let first = read(&f, move |conn| {
        lis_analytical::list_prep_batches(
            conn,
            lis_analytical::ListPrepBatchesInput {
                method_version_id: method,
                after: None,
                limit: 2,
            },
        )
    });
    assert_eq!(first.len(), 2);
    let after = first.last().unwrap().prep_batch_id;
    let next = read(&f, move |conn| {
        lis_analytical::list_prep_batches(
            conn,
            lis_analytical::ListPrepBatchesInput {
                method_version_id: method,
                after: Some(after),
                limit: 2,
            },
        )
    });
    let mut actual: Vec<_> = first
        .into_iter()
        .chain(next)
        .map(|b| b.prep_batch_id)
        .collect();
    actual.sort();
    ids.sort();
    assert_eq!(actual, ids);
    let reopened = read(&f, move |conn| lis_analytical::prep_batch(conn, batch));
    assert_eq!(reopened.steps.len(), 1);
    assert_eq!(reopened.steps[0].step_code, "extract");
}
