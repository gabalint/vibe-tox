use lis_core::Phi;

/// Numeric intermediate blocks are 0–9; E is the sole final block (p7).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlockId {
    Intermediate(u8),
    Final,
}

/// Session boundaries are distinct from per-sample requests and results.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SessionKind {
    QueryStart,
    QueryEnd,
    ResultsStart,
    ResultsEnd,
    TriStop,
}

/// The classification is preserved without promoting nonpatient results.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResultKind {
    Normal,
    Rerun,
    Stat,
    StatRerun,
    ReagentBlank,
    Calibration,
    QualityControl,
}
impl ResultKind {
    pub(crate) fn normal(self) -> bool {
        matches!(self, Self::Normal | Self::Stat)
    }
}

/// Each raw demographic field retains original padding and has redacted Debug.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Demographics {
    pub sex: Phi<Vec<u8>>,
    pub age: Phi<Vec<u8>>,
    pub month: Phi<Vec<u8>>,
    pub information: [Phi<Vec<u8>>; 6],
}

/// Fixed-width sample header shared across every block of one sample.
/// `reserved` is original sample number for reruns, dummy/control fields for
/// other kinds. QC sample-type is opaque dummy, never interpreted clinically.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SampleHeader {
    pub unit_no: Option<Phi<Vec<u8>>>,
    pub rack: Phi<Vec<u8>>,
    pub cup: Phi<Vec<u8>>,
    pub sample_type: Phi<Vec<u8>>,
    pub sample_no: Phi<Vec<u8>>,
    pub sample_id: Phi<Vec<u8>>,
    pub reserved: Phi<Vec<u8>>,
    pub demographics: Option<Demographics>,
}

/// One instrument TRI query; rerun does not imply an authorized laboratory order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Query {
    pub rerun: bool,
    pub header: SampleHeader,
}

/// A result item with exact value/flags; no interpretation of all-9 or LIH data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResultTest {
    pub code: Phi<Vec<u8>>,
    pub value: Phi<Vec<u8>>,
    pub flags: Phi<Vec<u8>>,
}

/// A validated result block, including exact classification and repeated header.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResultBlock {
    pub kind: ResultKind,
    pub header: SampleHeader,
    pub block: BlockId,
    pub tests: Vec<ResultTest>,
}

/// Parsed host TRI block; methods occur only for reruns.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TriBlock {
    pub rerun: bool,
    pub header: SampleHeader,
    pub block: BlockId,
    pub tests: Vec<crate::TriTest>,
}

/// Classified application text; byte provenance remains on the source Frame.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Message {
    Session {
        kind: SessionKind,
        unit_no: Option<Phi<Vec<u8>>>,
    },
    Query(Query),
    ResultBlock(ResultBlock),
    TriBlock(TriBlock),
}

/// Complete result text for one sample after all blocks have been checked.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SampleResults {
    pub kind: ResultKind,
    pub header: SampleHeader,
    pub tests: Vec<ResultTest>,
}
