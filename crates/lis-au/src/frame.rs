use crate::{AuConfig, AuError};
use lis_core::Phi;

/// Text-ending versus intermediate-block terminator (manual p7).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameEnd {
    Text,
    Block,
}

/// Exact wire and application text; debug formatting always redacts both.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Frame {
    pub wire: Phi<Vec<u8>>,
    pub text: Phi<Vec<u8>>,
    pub end: FrameEnd,
}

/// Transport event only; receiving a frame never implies clinical acceptance.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WireEvent {
    Frame(Frame),
    Ack,
    Nak,
}

/// Incremental decoder retaining at most the configured wire limit.
#[derive(Debug)]
pub struct StreamDecoder {
    config: AuConfig,
    pending: Phi<Vec<u8>>,
}

impl StreamDecoder {
    pub fn new(config: AuConfig) -> Result<Self, AuError> {
        config.validate()?;
        Ok(Self {
            config,
            pending: Phi::new(Vec::new()),
        })
    }
    /// Feeds arbitrary TCP/serial chunk boundaries, returning independent events.
    pub fn push(&mut self, bytes: &[u8]) -> Vec<Result<WireEvent, AuError>> {
        let mut events = Vec::new();
        for byte in bytes {
            match self.feed(*byte) {
                Ok(Some(event)) => events.push(Ok(event)),
                Ok(None) => {}
                Err(error) => {
                    // Preserve a potential new start after malformed partial framing.
                    self.resync(*byte);
                    events.push(Err(error));
                }
            }
        }
        events
    }
    /// Reports incomplete wire data at EOF; clears it for a fresh connection.
    pub fn finish(&mut self) -> Result<(), AuError> {
        let incomplete = !self.pending.expose().is_empty();
        self.pending = Phi::new(Vec::new());
        if incomplete {
            Err(AuError::Framing)
        } else {
            Ok(())
        }
    }
    /// True while an incomplete frame awaits more bytes.
    pub fn is_pending(&self) -> bool {
        !self.pending.expose().is_empty()
    }
    /// Clears wire state after quarantining an incomplete receipt.
    pub fn reset(&mut self) {
        self.pending = Phi::new(Vec::new());
    }
    /// Amount of retained wire data, excluding the returned event queue.
    pub fn buffered_len(&self) -> usize {
        self.pending.expose().len()
    }

    fn resync(&mut self, byte: u8) {
        if byte == self.config.framing.start[0] {
            self.pending = Phi::new(vec![byte]);
        }
    }
    fn feed(&mut self, byte: u8) -> Result<Option<WireEvent>, AuError> {
        let mut pending = std::mem::replace(&mut self.pending, Phi::new(Vec::new())).into_inner();
        if pending.is_empty() {
            if byte == 6 {
                return Ok(Some(WireEvent::Ack));
            }
            if byte == 21 {
                return Ok(Some(WireEvent::Nak));
            }
            if byte != self.config.framing.start[0] {
                return Err(AuError::Framing);
            }
        }
        pending.push(byte);
        if pending.len() > usize::from(self.config.framing.max_wire_len) {
            return Err(AuError::FrameTooLong);
        }
        let start_len = self.config.framing.start.len();
        if pending.len() <= start_len {
            if !self.config.framing.start.starts_with(&pending) {
                return Err(AuError::Framing);
            }
        } else if let Some((offset, end, end_len)) = self.termination(&pending)?
            && pending.len() == offset + end_len + usize::from(self.config.framing.bcc)
        {
            return self.complete(pending, offset, end, end_len).map(Some);
        }
        self.pending = Phi::new(pending);
        Ok(None)
    }

    fn termination(&self, bytes: &[u8]) -> Result<Option<(usize, FrameEnd, usize)>, AuError> {
        let start = self.config.framing.start.len();
        let Some(offset) = bytes[start..].iter().position(|b| !(32..=126).contains(b)) else {
            return Ok(None);
        };
        let offset = start + offset;
        let rest = &bytes[offset..];
        let (end, code) = if self.config.etb_enabled() && rest[0] == 23 {
            (FrameEnd::Block, &[23][..])
        } else {
            (FrameEnd::Text, self.config.framing.end.as_slice())
        };
        let compare_len = rest.len().min(code.len());
        if rest[..compare_len] != code[..compare_len] {
            return Err(AuError::Framing);
        }
        if rest.len() < code.len() {
            return Ok(None);
        }
        Ok(Some((offset, end, code.len())))
    }

    fn complete(
        &self,
        wire: Vec<u8>,
        offset: usize,
        end: FrameEnd,
        end_len: usize,
    ) -> Result<WireEvent, AuError> {
        let start = self.config.framing.start.len();
        if self.config.framing.bcc {
            let actual = wire[start..offset + end_len]
                .iter()
                .fold(0, |sum, b| sum ^ b);
            if wire[offset + end_len] != actual {
                return Err(AuError::Checksum);
            }
        }
        let text = Phi::new(wire[start..offset].to_vec());
        Ok(WireEvent::Frame(Frame {
            wire: Phi::new(wire),
            text,
            end,
        }))
    }
}

/// Encodes printable ASCII text and XOR BCC covering text plus terminator (p5).
pub fn encode_frame(config: &AuConfig, text: &[u8], end: FrameEnd) -> Result<Vec<u8>, AuError> {
    config.validate()?;
    if text.len() + config.overhead() > usize::from(config.framing.max_wire_len) {
        return Err(AuError::FrameTooLong);
    }
    if !text.iter().all(|b| (32..=126).contains(b)) {
        return Err(AuError::Character);
    }
    let terminator = match end {
        FrameEnd::Text => config.framing.end.as_slice(),
        FrameEnd::Block if config.etb_enabled() => &[23],
        FrameEnd::Block => config.framing.end.as_slice(),
    };
    let mut wire = config.framing.start.clone();
    wire.extend_from_slice(text);
    wire.extend_from_slice(terminator);
    if config.framing.bcc {
        wire.push(
            wire[config.framing.start.len()..]
                .iter()
                .fold(0, |sum, b| sum ^ b),
        );
    }
    if wire.len() > usize::from(config.framing.max_wire_len) {
        return Err(AuError::FrameTooLong);
    }
    Ok(wire)
}
