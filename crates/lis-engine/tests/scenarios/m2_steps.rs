//! Journey steps on the [`Lab`]: delivering an ASTM message through the
//! analyzer's real TCP listener (REQ-INT-009), waiting for the bounded
//! outbox import, accepting QC and submitting technical review. Expected
//! outcomes stay in the tests. Synthetic data only.

use std::io::{Read, Write};
use std::net::{Ipv4Addr, TcpStream};
use std::time::Duration;

use lis_analytical::{QcOutcome, QcScope};
use lis_catalog::AnalyteId;
use lis_core::CommandError;
use lis_engine::{
    AssessQcInput, GetRunQcInput, GetSpecimenResultsInput, ImportAttemptSummary,
    ListImportAttemptsInput, ReviewView, SpecimenResults, SubmitTechnicalReviewInput,
};
use lis_interfaces::ReceiptId;
use lis_results::{ResultLineView, ResultRevisionId, ReviewDecision};

use crate::m2_lab::{Lab, Role};

/// The synthetic messages of `fixtures/astm/m2/` (see its README).
pub(crate) const JOURNEY: &[u8] = include_bytes!("../../../../fixtures/astm/m2/journey.astm");
pub(crate) const JOURNEY_DUPLICATE: &[u8] =
    include_bytes!("../../../../fixtures/astm/m2/journey_duplicate.astm");
pub(crate) const CORRECTION: &[u8] = include_bytes!("../../../../fixtures/astm/m2/correction.astm");
pub(crate) const PARTIAL: &[u8] = include_bytes!("../../../../fixtures/astm/m2/partial.astm");
pub(crate) const UNKNOWN_SAMPLE: &[u8] =
    include_bytes!("../../../../fixtures/astm/m2/unknown_sample.astm");
pub(crate) const UNIT_MISMATCH: &[u8] =
    include_bytes!("../../../../fixtures/astm/m2/unit_mismatch.astm");
pub(crate) const MALFORMED_VALUE: &[u8] =
    include_bytes!("../../../../fixtures/astm/m2/malformed_value.astm");

const ENQ: u8 = 0x05;
const ACK: u8 = 0x06;
const EOT: u8 = 0x04;
const STX: u8 = 0x02;
const ETX: u8 = 0x03;
const CR: u8 = 0x0d;
const LF: u8 = 0x0a;

/// Bounded waits for the listener and the outbox worker, which run on their
/// own schedule (the dispatcher polls every two seconds).
const POLLS: usize = 400;
const POLL: Duration = Duration::from_millis(25);

/// A receipt artifact of synthetic records from the README's sender, in the
/// fixture format: each record ended by CR.
pub(crate) fn message(records: &[&str]) -> Vec<u8> {
    let mut artifact = b"H|\\^&|||SYNTH-LCMS^1.0|||||||P|LIS2-A2|20261012140000\r".to_vec();
    for record in records {
        artifact.extend_from_slice(record.as_bytes());
        artifact.push(CR);
    }
    artifact.extend_from_slice(b"L|1|N\r");
    artifact
}

/// One final frame per record (every synthetic record is under 240 bytes),
/// numbered from 1 modulo 8, with the uppercase hexadecimal checksum over
/// the frame number through ETX. `lis-astm` owns the production encoder, but
/// it is not a dev-dependency of `lis-engine`.
fn frames(artifact: &[u8]) -> Vec<Vec<u8>> {
    artifact
        .split(|byte| *byte == CR)
        .filter(|record| !record.is_empty())
        .enumerate()
        .map(|(index, record)| {
            let number = b'0' + u8::try_from((index + 1) % 8).unwrap();
            let mut frame = vec![STX, number];
            frame.extend_from_slice(record);
            frame.extend_from_slice(&[CR, ETX]);
            let sum = frame[1..].iter().fold(0_u8, |sum, b| sum.wrapping_add(*b));
            frame.extend_from_slice(format!("{sum:02X}").as_bytes());
            frame.extend_from_slice(&[CR, LF]);
            frame
        })
        .collect()
}

/// Sends one message as an analyzer would, requiring an ACK for the
/// enquiry and for every frame.
fn send(port: u16, artifact: &[u8]) {
    let mut stream = TcpStream::connect((Ipv4Addr::LOCALHOST, port)).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let mut reply = [0_u8; 1];
    stream.write_all(&[ENQ]).unwrap();
    stream.read_exact(&mut reply).unwrap();
    assert_eq!(reply[0], ACK, "enquiry not acknowledged");
    for frame in frames(artifact) {
        stream.write_all(&frame).unwrap();
        stream.read_exact(&mut reply).unwrap();
        assert_eq!(reply[0], ACK, "frame not acknowledged");
    }
    stream.write_all(&[EOT]).unwrap();
}

/// A completed receipt: its ID and the receipt it duplicates, if any.
pub(crate) struct Delivered {
    pub(crate) receipt_id: ReceiptId,
    pub(crate) duplicate_of: Option<ReceiptId>,
}

impl Lab {
    /// Delivers `artifact` through the listener and waits for its durable
    /// receipt. Receipts have no engine query, so this reads the stored row.
    pub(crate) async fn deliver(&self, artifact: &[u8]) -> Delivered {
        let before = self.complete_receipts().len();
        let (port, bytes) = (self.port, artifact.to_vec());
        tokio::task::spawn_blocking(move || send(port, &bytes))
            .await
            .unwrap();
        for _ in 0..POLLS {
            let receipts = self.complete_receipts();
            if receipts.len() > before {
                let (id, duplicate_of) = receipts.last().unwrap().clone();
                return Delivered {
                    receipt_id: id.parse().unwrap(),
                    duplicate_of: duplicate_of.map(|id| id.parse().unwrap()),
                };
            }
            tokio::time::sleep(POLL).await;
        }
        panic!("the listener completed no receipt");
    }

    fn complete_receipts(&self) -> Vec<(String, Option<String>)> {
        self.t()
            .conn()
            .prepare(
                "SELECT id, duplicate_of FROM ifc_receipts WHERE state = 'complete' ORDER BY id",
            )
            .unwrap()
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
    }

    /// Delivers `artifact` and waits for the result-import worker's attempt.
    pub(crate) async fn import(&self, artifact: &[u8]) -> ImportAttemptSummary {
        let delivered = self.deliver(artifact).await;
        assert_eq!(delivered.duplicate_of, None);
        self.attempt(delivered.receipt_id, 1).await
    }

    /// Waits for import attempt `attempt_no` of `receipt_id`.
    pub(crate) async fn attempt(
        &self,
        receipt_id: ReceiptId,
        attempt_no: i64,
    ) -> ImportAttemptSummary {
        for _ in 0..POLLS {
            let found = self
                .attempts()
                .await
                .into_iter()
                .find(|a| a.receipt_id == receipt_id && a.attempt_no == attempt_no);
            if let Some(attempt) = found {
                return attempt;
            }
            tokio::time::sleep(POLL).await;
        }
        panic!("the result-import worker recorded no attempt");
    }

    /// Every import attempt of the analyzer, as the reviewer lists them.
    pub(crate) async fn attempts(&self) -> Vec<ImportAttemptSummary> {
        self.sign_in(Role::Reviewer).await;
        self.engine()
            .list_import_attempts(ListImportAttemptsInput {
                after_import_attempt_id: None,
                instrument_id: Some(self.instrument),
                limit: 100,
                before: None,
            })
            .await
            .unwrap()
    }

    /// Accepts run × analyte QC for `analyte` on the current observations.
    pub(crate) async fn assess_qc(
        &self,
        analyte: AnalyteId,
        outcome: QcOutcome,
    ) -> Result<lis_engine::QcAssessmentView, CommandError> {
        self.sign_in(Role::Quality).await;
        let qc = self
            .engine()
            .get_run_qc(GetRunQcInput { run_id: self.run })
            .await
            .unwrap();
        let entry = qc
            .analytes
            .iter()
            .find(|a| a.analyte_id == analyte)
            .unwrap();
        let current = entry.assessments.current.as_ref();
        self.engine()
            .assess_qc(AssessQcInput {
                scope: QcScope::RunAnalyte,
                run_id: Some(self.run),
                analyte_id: Some(analyte),
                prep_batch_id: None,
                outcome,
                evidence_observation_ids: entry
                    .observations
                    .iter()
                    .filter(|o| !o.superseded)
                    .map(|o| o.observation_id)
                    .collect(),
                supersedes_assessment_id: current.map(|a| a.assessment_id),
                reason: current.map(|_| "Synthetic QC review".to_owned()),
            })
            .await
    }

    /// Accepts QC for both drug analytes.
    pub(crate) async fn accept_qc(&self) {
        for analyte in [self.analytes.mor, self.analytes.oxy] {
            self.assess_qc(analyte, QcOutcome::Accepted).await.unwrap();
        }
    }

    /// The reviewer's audited view of sample `index`.
    pub(crate) async fn results(&self, index: usize) -> SpecimenResults {
        self.sign_in(Role::Reviewer).await;
        self.engine()
            .get_specimen_results(GetSpecimenResultsInput {
                specimen_id: self.samples[index].specimen_id,
            })
            .await
            .unwrap()
    }

    /// Approves `revisions` of sample `index`.
    pub(crate) async fn approve(
        &self,
        index: usize,
        revisions: Vec<ResultRevisionId>,
    ) -> Result<ReviewView, CommandError> {
        self.sign_in(Role::Reviewer).await;
        self.engine()
            .submit_technical_review(SubmitTechnicalReviewInput {
                specimen_id: self.samples[index].specimen_id,
                decision: ReviewDecision::Approved,
                result_revision_ids: revisions,
                reason: None,
            })
            .await
    }

    /// Approves every current revision of sample `index`.
    pub(crate) async fn approve_all(&self, index: usize) -> Result<ReviewView, CommandError> {
        let revisions = current_revisions(&self.results(index).await);
        self.approve(index, revisions).await
    }
}

/// The line for the analyte with catalog `code`.
pub(crate) fn line<'a>(results: &'a SpecimenResults, code: &str) -> &'a ResultLineView {
    results
        .lines
        .iter()
        .find(|line| line.analyte_code == code)
        .unwrap_or_else(|| panic!("no {code} line"))
}

pub(crate) fn current_revisions(results: &SpecimenResults) -> Vec<ResultRevisionId> {
    results
        .lines
        .iter()
        .filter_map(|line| line.current.as_ref().map(|c| c.revision_id))
        .collect()
}

/// The gate codes a refusal carries.
pub(crate) fn gate_codes(error: &CommandError) -> Vec<String> {
    serde_json::to_value(error).unwrap()["gateReasons"]
        .as_array()
        .unwrap()
        .iter()
        .map(|gate| gate["code"].as_str().unwrap().to_owned())
        .collect()
}
