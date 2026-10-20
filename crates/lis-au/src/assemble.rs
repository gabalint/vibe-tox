use crate::{AuError, BlockId, ResultBlock, SampleResults};

/// Duplicate means the last intermediate block is byte-field-identical.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AssemblyOutcome {
    Pending,
    Duplicate,
    Complete(Box<SampleResults>),
}

/// Holds only one in-progress sample, bounded by blocks 0–9 plus E (manual p7).
/// A new sample must not interrupt a partial one. On any error the caller must
/// quarantine the receipt and explicitly reset or terminate the session.
#[derive(Debug, Default)]
pub struct ResultAssembler {
    sample: Option<SampleResults>,
    last: Option<ResultBlock>,
    next: u8,
}
impl ResultAssembler {
    pub fn new() -> Self {
        Self::default()
    }
    /// True while a sample is waiting for its final E block.
    pub fn is_pending(&self) -> bool {
        self.sample.is_some()
    }
    /// Refuses an EOF/session boundary while a sample remains incomplete.
    pub fn finish(&self) -> Result<(), AuError> {
        if self.is_pending() {
            Err(AuError::IncompleteSample)
        } else {
            Ok(())
        }
    }
    /// Drops incomplete assembly after its receipts have been quarantined.
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn push(&mut self, block: ResultBlock) -> Result<AssemblyOutcome, AuError> {
        if block.tests.is_empty() || block.tests.len() > 102 || !block_is_bounded(&block) {
            return Err(AuError::Length);
        }
        if let Some(sample) = &self.sample
            && (sample.kind != block.kind || sample.header != block.header)
        {
            return Err(AuError::HeaderConflict);
        }
        if let Some(last) = &self.last
            && last.block == block.block
        {
            if last != &block {
                return Err(AuError::BlockConflict);
            }
            return Ok(AssemblyOutcome::Duplicate);
        }
        match block.block {
            BlockId::Intermediate(number) if number > 9 => Err(AuError::TooManyBlocks),
            BlockId::Intermediate(number) if number != self.next => Err(AuError::BlockSequence),
            BlockId::Intermediate(_) => self.append_intermediate(block),
            BlockId::Final => self.complete(block),
        }
    }
    fn append_intermediate(&mut self, block: ResultBlock) -> Result<AssemblyOutcome, AuError> {
        if self.next > 9 {
            return Err(AuError::TooManyBlocks);
        }
        self.next += 1;
        let sample = self.sample.get_or_insert_with(|| SampleResults {
            kind: block.kind,
            header: block.header.clone(),
            tests: Vec::new(),
        });
        sample.tests.extend(block.tests.iter().cloned());
        self.last = Some(block);
        Ok(AssemblyOutcome::Pending)
    }
    fn complete(&mut self, block: ResultBlock) -> Result<AssemblyOutcome, AuError> {
        let mut sample = self.sample.take().unwrap_or_else(|| SampleResults {
            kind: block.kind,
            header: block.header,
            tests: Vec::new(),
        });
        sample.tests.extend(block.tests);
        self.last = None;
        self.next = 0;
        Ok(AssemblyOutcome::Complete(Box::new(sample)))
    }
}

fn block_is_bounded(block: &ResultBlock) -> bool {
    let header = &block.header;
    let mut fields = vec![
        &header.rack,
        &header.cup,
        &header.sample_type,
        &header.sample_no,
        &header.sample_id,
        &header.reserved,
    ];
    if let Some(unit) = &header.unit_no {
        fields.push(unit);
    }
    if let Some(demographics) = &header.demographics {
        fields.extend([&demographics.sex, &demographics.age, &demographics.month]);
        fields.extend(demographics.information.iter());
    }
    if fields.iter().any(|field| field.expose().len() > 20) {
        return false;
    }
    block.tests.iter().all(|test| {
        test.code.expose().len() == 2
            && matches!(test.value.expose().len(), 6 | 9)
            && test.flags.expose().len() == 2
    })
}
