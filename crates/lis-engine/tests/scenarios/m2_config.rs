//! Configuration of the M2 journey laboratory (see [`crate::m2_lab`]):
//! catalog, validity policy, analyzer, source registration and control
//! lots, drafted by the administrator and approved by a second person, the
//! quality manager. Values follow `fixtures/astm/m2/README.md`. Synthetic
//! data only.

use std::net::{IpAddr, Ipv4Addr};

use lis_analytical::{
    InstrumentStatus, LotId, MaterialKind, RevisionIndicator, SampleIdField, SourceTestCodeInput,
};
use lis_catalog::{
    AnalyteId, ComponentInput, ComponentPurpose, MethodAnalyte, MethodCertainty,
    MethodVersionContent, MethodVersionId, OrderableKind, OrderableVersionContent,
    OrderableVersionId, OutputScale, ThresholdRounding, ThresholdRule, ValidityAction,
    ValidityCategory, ValidityPolicyContent,
};
use lis_core::{DecimalText, InstrumentId};
use lis_engine::{
    ApproveMethodVersion, ApproveOrderableVersion, ApproveSourceInput,
    ApproveValidityPolicyVersion, CodedName, CreateAnalyte, CreateOrderable, DraftMethodVersion,
    DraftOrderableVersion, DraftValidityPolicyVersion, Engine, ListSourceRegistrationsInput,
    OrderableId, RegisterInstrument, RegisterLotInput, RegisterSourceInput,
    SetInstrumentStatusInput, SpecimenTypeId,
};
use lis_interfaces::ConnectionSettings;
use lis_store::testing::local_port;

use crate::m2_lab::{Desk, Role};

/// The README's specimen type and unit.
pub(crate) const URINE: &str = "SYN-UR";
const UNIT: &str = "ng/mL";

/// The analytes of the method version, by README test code.
#[derive(Clone, Copy)]
pub(crate) struct Analytes {
    pub(crate) mor: AnalyteId,
    pub(crate) oxy: AnalyteId,
    /// A validity analyte (synthetic creatinine) for the validity component.
    pub(crate) cre: AnalyteId,
}

/// The approved method versions.
#[derive(Clone, Copy)]
pub(crate) struct Methods {
    /// `SYN-MOR` and `SYN-OXY`, the README's configuration.
    pub(crate) drugs: MethodVersionId,
    /// `SYN-CRE`, a specimen-validity measurement without a cutoff.
    pub(crate) validity: MethodVersionId,
}

/// The approved orderables: `(orderable, approved version)`.
#[derive(Clone, Copy)]
pub(crate) struct Orderables {
    /// `SYN-MOR` and `SYN-OXY`, first line; reflex to `confirmation` authorized.
    pub(crate) panel: (OrderableId, OrderableVersionId),
    /// `SYN-MOR` only, as a confirmation.
    pub(crate) confirmation: (OrderableId, OrderableVersionId),
    /// The panel plus a validity component (`SYN-CRE`).
    pub(crate) with_validity: (OrderableId, OrderableVersionId),
}

/// Configuration IDs before orders and the run exist.
pub(crate) struct Configured {
    pub(crate) analytes: Analytes,
    pub(crate) methods: Methods,
    pub(crate) orderables: Orderables,
    pub(crate) instrument: InstrumentId,
    pub(crate) port: u16,
    pub(crate) lots: [LotId; 2],
}

/// Draft configuration by the administrator, approvals and readiness by
/// the quality manager.
pub(crate) async fn configure(desk: &Desk, indicator: RevisionIndicator) -> Configured {
    desk.sign_in(Role::Admin).await;
    let e = desk.engine();
    let urine = e.create_specimen_type(coded(URINE)).await.unwrap();
    let analytes = Analytes {
        mor: analyte(e, "SYN-MOR").await,
        oxy: analyte(e, "SYN-OXY").await,
        cre: analyte(e, "SYN-CRE").await,
    };
    let methods = Methods {
        drugs: draft_method(e, "SYN-LCMS-M", urine, drug_analytes(analytes)).await,
        validity: draft_method(e, "SYN-CRE-M", urine, vec![creatinine(analytes.cre)]).await,
    };
    let test_codes = [
        ("SYN-MOR", analytes.mor),
        ("SYN-OXY", analytes.oxy),
        ("SYN-CRE", analytes.cre),
    ];
    let (instrument, port) = register_analyzer(desk, "SYNTH-LCMS-01", &test_codes, indicator).await;
    let policy = e
        .draft_validity_policy_version(DraftValidityPolicyVersion {
            specimen_type_id: urine,
            content: validity_policy(),
        })
        .await
        .unwrap()
        .id;
    approve_configuration(desk, methods, policy).await;
    let orderables = approved_orderables(desk, methods, analytes).await;
    let lots = ready_analyzer(desk, instrument).await;
    Configured {
        analytes,
        methods,
        orderables,
        instrument,
        port,
        lots,
    }
}

/// The enabled TCP analyzer `code`, sender `SYNTH-LCMS` 1.0, and its draft source
/// registration mapping `test_codes`.
pub(crate) async fn register_analyzer(
    desk: &Desk,
    code: &str,
    test_codes: &[(&str, AnalyteId)],
    indicator: RevisionIndicator,
) -> (InstrumentId, u16) {
    let e = desk.engine();
    let port = local_port();
    let instrument = e
        .register_instrument(RegisterInstrument {
            code: code.to_owned(),
            name: "Synthetic LC-MS/MS".to_owned(),
            connection: ConnectionSettings::AstmTcp {
                bind_address: IpAddr::V4(Ipv4Addr::LOCALHOST),
                port,
                allowed_peers: vec![IpAddr::V4(Ipv4Addr::LOCALHOST)],
            },
            enabled: true,
        })
        .await
        .unwrap()
        .instrument
        .id;
    e.register_source(RegisterSourceInput {
        au_profile: None,
        instrument_id: instrument,
        sender_name: "SYNTH-LCMS".to_owned(),
        sender_version: "1.0".to_owned(),
        sample_id_field: SampleIdField::SpecimenId,
        revision_indicator: indicator,
        reports_final_concentration: false,
        test_codes: test_codes
            .iter()
            .map(|(source_test_code, analyte_id)| SourceTestCodeInput {
                source_test_code: (*source_test_code).to_owned(),
                analyte_id: *analyte_id,
            })
            .collect(),
    })
    .await
    .unwrap();
    (instrument, port)
}

/// A second person approves the methods and the validity policy.
async fn approve_configuration(
    desk: &Desk,
    methods: Methods,
    policy: lis_catalog::ValidityPolicyVersionId,
) {
    desk.sign_in(Role::Quality).await;
    let e = desk.engine();
    for method_version_id in [methods.drugs, methods.validity] {
        e.approve_method_version(ApproveMethodVersion {
            external_approval_reference: None,
            method_version_id,
            expected_revision: 1,
            self_approval: false,
        })
        .await
        .unwrap();
    }
    e.approve_validity_policy_version(ApproveValidityPolicyVersion {
        policy_version_id: policy,
        expected_revision: 1,
        self_approval: false,
    })
    .await
    .unwrap();
}

async fn approved_orderables(desk: &Desk, methods: Methods, analytes: Analytes) -> Orderables {
    let analytical = |analyte_ids| ComponentInput {
        method_version_id: methods.drugs,
        purpose: ComponentPurpose::Analytical,
        analyte_ids,
    };
    let validity = ComponentInput {
        method_version_id: methods.validity,
        purpose: ComponentPurpose::Validity,
        analyte_ids: vec![analytes.cre],
    };
    let panel = vec![analytical(vec![analytes.mor, analytes.oxy])];
    let mut with_validity = panel.clone();
    with_validity.push(validity);
    Orderables {
        panel: orderable(desk, "SYN-PANEL", panel).await,
        confirmation: orderable(desk, "SYN-CONF", vec![analytical(vec![analytes.mor])]).await,
        with_validity: orderable(desk, "SYN-PANEL-V", with_validity).await,
    }
}

/// Drafted by the administrator, approved by the quality manager.
pub(crate) async fn orderable(
    desk: &Desk,
    code: &str,
    components: Vec<ComponentInput>,
) -> (OrderableId, OrderableVersionId) {
    desk.sign_in(Role::Admin).await;
    let e = desk.engine();
    let orderable_id = e
        .create_orderable(CreateOrderable {
            code: code.to_owned(),
            name: format!("Synthetic {code}"),
            kind: if components.len() > 1 {
                OrderableKind::Panel
            } else {
                OrderableKind::Test
            },
        })
        .await
        .unwrap();
    let version = e
        .draft_orderable_version(DraftOrderableVersion {
            orderable_id,
            content: OrderableVersionContent { components },
        })
        .await
        .unwrap()
        .id;
    desk.sign_in(Role::Quality).await;
    e.approve_orderable_version(ApproveOrderableVersion {
        orderable_version_id: version,
        expected_revision: 1,
        self_approval: false,
    })
    .await
    .unwrap();
    (orderable_id, version)
}

/// Approves the source registration, puts the analyzer in service and
/// registers the two control lots.
async fn ready_analyzer(desk: &Desk, instrument: InstrumentId) -> [LotId; 2] {
    approve_analyzer(desk, instrument).await;
    let e = desk.engine();
    [
        control_lot(e, "LOW", "QC-LOW-SYN").await,
        control_lot(e, "HIGH", "QC-HIGH-SYN").await,
    ]
}

/// The quality manager approves the analyzer's source registration and puts it in
/// service.
pub(crate) async fn approve_analyzer(desk: &Desk, instrument: InstrumentId) {
    desk.sign_in(Role::Quality).await;
    let e = desk.engine();
    let registration = e
        .list_source_registrations(ListSourceRegistrationsInput {
            instrument_id: instrument,
        })
        .await
        .unwrap()[0]
        .registration_id;
    e.approve_source(ApproveSourceInput {
        registration_id: registration,
        expected_revision: 1,
        self_approval: false,
    })
    .await
    .unwrap();
    e.set_instrument_status(SetInstrumentStatusInput {
        instrument_id: instrument,
        status: InstrumentStatus::InService,
        reason: "Synthetic daily maintenance".to_owned(),
    })
    .await
    .unwrap();
}

fn coded(code: &str) -> CodedName {
    CodedName {
        code: code.to_owned(),
        name: format!("Synthetic {code}"),
    }
}

pub(crate) async fn analyte(e: &Engine, code: &str) -> AnalyteId {
    e.create_analyte(CreateAnalyte {
        code: code.to_owned(),
        name: format!("Synthetic {code}"),
        loinc_code: None,
        drug_class_code: None,
    })
    .await
    .unwrap()
}

pub(crate) async fn draft_method(
    e: &Engine,
    code: &str,
    specimen_type_id: SpecimenTypeId,
    analytes: Vec<MethodAnalyte>,
) -> MethodVersionId {
    let method_id = e.create_method(coded(code)).await.unwrap();
    let content = MethodVersionContent {
        report_notes: None,
        specimen_type_id,
        certainty: MethodCertainty::Definitive,
        output_scale: OutputScale::Quantitative,
        threshold_rule: ThresholdRule::AtOrAbove,
        threshold_rounding: ThresholdRounding::BeforeRounding,
        analytes,
    };
    e.draft_method_version(DraftMethodVersion { method_id, content })
        .await
        .unwrap()
        .id
}

/// The README's suggested configuration: unit `ng/mL`, cutoff `50` with
/// the method's `at_or_above` and `before_rounding`, display scale 1.
fn drug_analytes(analytes: Analytes) -> Vec<MethodAnalyte> {
    [analytes.mor, analytes.oxy]
        .into_iter()
        .map(|analyte_id| MethodAnalyte {
            cutoff: Some(DecimalText::parse("50").unwrap()),
            ..creatinine(analyte_id)
        })
        .map(|analyte| MethodAnalyte {
            unit: Some(UNIT.to_owned()),
            ..analyte
        })
        .collect()
}

/// A synthetic validity measurement in `mg/dL` with no cutoff.
fn creatinine(analyte_id: AnalyteId) -> MethodAnalyte {
    MethodAnalyte {
        reference_interval: None,
        range_policy: lis_catalog::RangePolicy::ReportOnly,
        analyte_id,
        unit: Some("mg/dL".to_owned()),
        cutoff: None,
        limit_of_detection: None,
        limit_of_quantitation: None,
        reportable_low: None,
        reportable_high: None,
        display_scale: Some(1),
    }
}

fn validity_policy() -> ValidityPolicyContent {
    ValidityPolicyContent {
        categories: vec![ValidityCategory {
            code: "SYN-VALID".to_owned(),
            label: "Synthetic valid".to_owned(),
            action: ValidityAction::None,
            qualifier_text: None,
        }],
    }
}

async fn control_lot(e: &Engine, level: &str, sample_label: &str) -> LotId {
    e.register_lot(RegisterLotInput {
        material_kind: MaterialKind::Control,
        product_name: "Synthetic urine control".to_owned(),
        lot_number: format!("SYN-{level}-1"),
        level_text: Some(level.to_owned()),
        sample_label: Some(sample_label.to_owned()),
        expires_on: "2027-12-31".to_owned(),
    })
    .await
    .unwrap()
    .lot_id
}
