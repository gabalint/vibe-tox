use crate::driver_contract::{
    AbortReason, Action, ControlResponse, DEFAULT_MAX_FRAMES_PER_TRANSFER,
    DEFAULT_MAX_MESSAGE_BYTES, DEFAULT_MAX_WIRE_BYTES_PER_TRANSFER, DriverError, FrameId,
    RECEIVE_TIMEOUT, ReceiveEvent, TimerId,
};
use crate::frame::{CR, ENQ, EOT, Frame, FrameEnd, LF, STX, parse_frame};

const MAX_WIRE_FRAME_BYTES: usize = 247;
const MAX_FAILED_ATTEMPTS: u8 = 6;

#[derive(Clone, Debug)]
struct PendingFrame {
    id: FrameId,
    frame: Frame,
    accepted_text_bytes: usize,
    accepted_frames: u32,
    accepted_wire_bytes: usize,
}

#[derive(Clone, Debug)]
struct AcceptedFrame {
    number: u8,
    raw: Vec<u8>,
}

/// Pure receive-side low-level driver with durable frame acknowledgement gating.
#[derive(Debug)]
pub struct ReceiveDriver {
    receiving: bool,
    expected_number: u8,
    failed_attempts: u8,
    wire_buffer: Vec<u8>,
    current_record: Vec<u8>,
    record_continues: bool,
    records: Vec<Vec<u8>>,
    accepted_text_bytes: usize,
    accepted_frames: u32,
    accepted_wire_bytes: usize,
    max_message_bytes: usize,
    max_frames: u32,
    max_wire_bytes: usize,
    pending: Option<PendingFrame>,
    last_accepted: Option<AcceptedFrame>,
    next_frame_id: u64,
    active_timer: Option<TimerId>,
    next_timer_id: u64,
}

impl Default for ReceiveDriver {
    fn default() -> Self {
        Self::new(DEFAULT_MAX_MESSAGE_BYTES)
    }
}

impl ReceiveDriver {
    /// Constructs an idle receiver with an explicit whole-transfer text limit.
    #[must_use]
    pub fn new(max_message_bytes: usize) -> Self {
        Self {
            receiving: false,
            expected_number: 1,
            failed_attempts: 0,
            wire_buffer: Vec::new(),
            current_record: Vec::new(),
            record_continues: false,
            records: Vec::new(),
            accepted_text_bytes: 0,
            accepted_frames: 0,
            accepted_wire_bytes: 0,
            max_message_bytes,
            max_frames: DEFAULT_MAX_FRAMES_PER_TRANSFER,
            max_wire_bytes: DEFAULT_MAX_WIRE_BYTES_PER_TRANSFER,
            pending: None,
            last_accepted: None,
            next_frame_id: 1,
            active_timer: None,
            next_timer_id: 1,
        }
    }

    /// Applies one event and returns ordered external actions.
    pub fn handle(&mut self, event: ReceiveEvent) -> Result<Vec<Action>, DriverError> {
        let mut actions = Vec::new();
        match event {
            ReceiveEvent::Bytes(bytes) => {
                if self.buffer_bytes(&bytes, &mut actions) {
                    self.process_wire(&mut actions)?;
                }
            }
            ReceiveEvent::Persisted(frame_id) => {
                self.handle_persisted(frame_id, &mut actions)?;
                self.process_wire(&mut actions)?;
            }
            ReceiveEvent::TimerExpired(timer_id) => {
                self.handle_timeout(timer_id, &mut actions);
            }
        }
        Ok(actions)
    }

    fn buffer_bytes(&mut self, bytes: &[u8], actions: &mut Vec<Action>) -> bool {
        let buffered_limit = self
            .max_message_bytes
            .saturating_mul(2)
            .max(MAX_WIRE_FRAME_BYTES);
        let Some(buffered_size) = self.wire_buffer.len().checked_add(bytes.len()) else {
            self.abort(AbortReason::InputBufferExceeded, actions);
            return false;
        };
        if buffered_size > buffered_limit {
            self.abort(AbortReason::InputBufferExceeded, actions);
            return false;
        }
        self.wire_buffer.extend_from_slice(bytes);
        true
    }

    fn process_wire(&mut self, actions: &mut Vec<Action>) -> Result<(), DriverError> {
        loop {
            if self.pending.is_some() || self.wire_buffer.is_empty() {
                return Ok(());
            }
            if !self.receiving {
                self.process_idle_byte(actions)?;
                continue;
            }
            if self.wire_buffer[0] == EOT {
                self.wire_buffer.remove(0);
                self.finish_transfer(actions);
                continue;
            }
            if self.wire_buffer[0] != STX {
                self.wire_buffer.clear();
                self.reject_attempt(actions)?;
                return Ok(());
            }
            let Some(frame_length) = complete_frame_length(&self.wire_buffer) else {
                self.reject_oversize_incomplete_frame(actions)?;
                return Ok(());
            };
            let raw: Vec<u8> = self.wire_buffer.drain(..frame_length).collect();
            self.process_frame(&raw, actions)?;
        }
    }

    fn reject_oversize_incomplete_frame(
        &mut self,
        actions: &mut Vec<Action>,
    ) -> Result<(), DriverError> {
        if self.wire_buffer.len() <= MAX_WIRE_FRAME_BYTES {
            return Ok(());
        }
        self.wire_buffer.clear();
        self.reject_attempt(actions)
    }

    fn process_idle_byte(&mut self, actions: &mut Vec<Action>) -> Result<(), DriverError> {
        let byte = self.wire_buffer.remove(0);
        if byte != ENQ {
            return Ok(());
        }
        self.receiving = true;
        self.expected_number = 1;
        self.failed_attempts = 0;
        self.current_record.clear();
        self.record_continues = false;
        self.records.clear();
        self.accepted_text_bytes = 0;
        self.accepted_frames = 0;
        self.accepted_wire_bytes = 0;
        self.pending = None;
        self.last_accepted = None;
        actions.push(Action::Send(ControlResponse::Ack));
        self.restart_timer(actions)
    }

    fn process_frame(&mut self, raw: &[u8], actions: &mut Vec<Action>) -> Result<(), DriverError> {
        let Ok(frame) = parse_frame(raw) else {
            self.reject_attempt(actions)?;
            return Ok(());
        };

        if self.is_last_accepted_duplicate(&frame) {
            // REQ-INT-003: the prior persistence confirmation proves durability.
            self.failed_attempts = 0;
            actions.push(Action::Send(ControlResponse::Ack));
            self.restart_timer(actions)?;
            return Ok(());
        }
        if frame.number() != self.expected_number {
            self.reject_attempt(actions)?;
            return Ok(());
        }
        if !self.record_continues && !self.current_record.is_empty() {
            self.abort(AbortReason::IncompleteRecord, actions);
            return Ok(());
        }
        if frame.text().is_empty() {
            self.abort(AbortReason::EmptyFrame, actions);
            return Ok(());
        }
        let Some(new_frame_count) = self.accepted_frames.checked_add(1) else {
            self.abort(AbortReason::FrameLimitExceeded, actions);
            return Ok(());
        };
        if new_frame_count > self.max_frames {
            self.abort(AbortReason::FrameLimitExceeded, actions);
            return Ok(());
        }
        let Some(new_wire_bytes) = self.accepted_wire_bytes.checked_add(frame.raw().len()) else {
            self.abort(AbortReason::WireBytesExceeded, actions);
            return Ok(());
        };
        if new_wire_bytes > self.max_wire_bytes {
            self.abort(AbortReason::WireBytesExceeded, actions);
            return Ok(());
        }
        let Some(new_size) = self.accepted_text_bytes.checked_add(frame.text().len()) else {
            self.abort(AbortReason::MessageTooLarge, actions);
            return Ok(());
        };
        if new_size > self.max_message_bytes {
            self.abort(AbortReason::MessageTooLarge, actions);
            return Ok(());
        }

        let frame_id = self.take_frame_id()?;
        actions.push(Action::PersistFrame {
            frame_id,
            bytes: frame.raw().to_vec(),
        });
        self.pending = Some(PendingFrame {
            id: frame_id,
            frame,
            accepted_text_bytes: new_size,
            accepted_frames: new_frame_count,
            accepted_wire_bytes: new_wire_bytes,
        });
        self.restart_timer(actions)
    }

    fn handle_persisted(
        &mut self,
        frame_id: FrameId,
        actions: &mut Vec<Action>,
    ) -> Result<(), DriverError> {
        let Some(pending) = self.pending.take() else {
            return Err(DriverError::UnknownFrameId);
        };
        if pending.id != frame_id {
            self.pending = Some(pending);
            return Err(DriverError::UnknownFrameId);
        }

        self.accepted_text_bytes = pending.accepted_text_bytes;
        self.accepted_frames = pending.accepted_frames;
        self.accepted_wire_bytes = pending.accepted_wire_bytes;
        self.current_record.extend_from_slice(pending.frame.text());
        self.record_continues = pending.frame.end() == FrameEnd::Intermediate;
        self.extract_complete_records();
        self.expected_number = (pending.frame.number() + 1) % 8;
        self.failed_attempts = 0;
        let accepted = AcceptedFrame {
            number: pending.frame.number(),
            raw: pending.frame.raw().to_vec(),
        };
        self.discard_buffered_duplicates(&accepted);
        self.last_accepted = Some(accepted);
        // REQ-INT-009: this is the only frame-acceptance ACK path.
        actions.push(Action::Send(ControlResponse::Ack));
        self.restart_timer(actions)
    }

    fn extract_complete_records(&mut self) {
        while let Some(end) = self.current_record.iter().position(|byte| *byte == CR) {
            let mut remainder = self.current_record.split_off(end + 1);
            let _ = self.current_record.pop();
            self.records.push(std::mem::take(&mut self.current_record));
            self.current_record.append(&mut remainder);
        }
    }

    fn discard_buffered_duplicates(&mut self, accepted: &AcceptedFrame) {
        while self.wire_buffer.starts_with(&accepted.raw) {
            self.wire_buffer.drain(..accepted.raw.len());
        }
    }

    fn finish_transfer(&mut self, actions: &mut Vec<Action>) {
        if self.record_continues || !self.current_record.is_empty() {
            self.abort(AbortReason::IncompleteRecord, actions);
            return;
        }
        if self.records.is_empty() {
            self.abort(AbortReason::EmptyTransfer, actions);
            return;
        }

        let records = std::mem::take(&mut self.records);
        self.reset_transfer();
        actions.push(Action::StopTimer);
        actions.push(Action::MessageComplete(records));
    }

    fn reject_attempt(&mut self, actions: &mut Vec<Action>) -> Result<(), DriverError> {
        self.failed_attempts = self.failed_attempts.saturating_add(1);
        actions.push(Action::Send(ControlResponse::Nak));
        if self.failed_attempts >= MAX_FAILED_ATTEMPTS {
            self.abort(AbortReason::RetryLimitExceeded, actions);
            return Ok(());
        }
        self.restart_timer(actions)
    }

    fn handle_timeout(&mut self, timer_id: TimerId, actions: &mut Vec<Action>) {
        if self.receiving && self.active_timer == Some(timer_id) {
            self.abort(AbortReason::ReceiveTimeout, actions);
        }
    }

    fn abort(&mut self, reason: AbortReason, actions: &mut Vec<Action>) {
        self.reset_transfer();
        actions.push(Action::StopTimer);
        actions.push(Action::TransferAborted(reason));
    }

    fn reset_transfer(&mut self) {
        self.receiving = false;
        self.expected_number = 1;
        self.failed_attempts = 0;
        self.wire_buffer.clear();
        self.current_record.clear();
        self.record_continues = false;
        self.records.clear();
        self.accepted_text_bytes = 0;
        self.accepted_frames = 0;
        self.accepted_wire_bytes = 0;
        self.pending = None;
        self.last_accepted = None;
        self.active_timer = None;
    }

    fn restart_timer(&mut self, actions: &mut Vec<Action>) -> Result<(), DriverError> {
        let timer_id = TimerId(self.next_timer_id);
        self.next_timer_id = self
            .next_timer_id
            .checked_add(1)
            .ok_or(DriverError::TimerIdExhausted)?;
        self.active_timer = Some(timer_id);
        actions.push(Action::StartTimer {
            timer_id,
            duration: RECEIVE_TIMEOUT,
        });
        Ok(())
    }

    fn take_frame_id(&mut self) -> Result<FrameId, DriverError> {
        let frame_id = FrameId(self.next_frame_id);
        self.next_frame_id = self
            .next_frame_id
            .checked_add(1)
            .ok_or(DriverError::FrameIdExhausted)?;
        Ok(frame_id)
    }

    fn is_last_accepted_duplicate(&self, frame: &Frame) -> bool {
        self.last_accepted.as_ref().is_some_and(|accepted| {
            frame.number() == accepted.number && frame.raw() == accepted.raw
        })
    }
}

fn complete_frame_length(bytes: &[u8]) -> Option<usize> {
    bytes
        .windows(2)
        .position(|pair| pair == [CR, LF])
        .map(|index| index + 2)
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;
    use crate::frame::{FrameEnd, encode_frame, encode_records};

    fn begin(driver: &mut ReceiveDriver) -> Vec<Action> {
        driver
            .handle(ReceiveEvent::Bytes(vec![ENQ]))
            .expect("ENQ is handled")
    }

    fn persistence_id(actions: &[Action]) -> FrameId {
        actions
            .iter()
            .find_map(Action::persistence_id)
            .expect("persistence action exists")
    }

    fn persist_first_frame(driver: &mut ReceiveDriver, first: Vec<u8>) {
        let _ = begin(driver);
        let actions = driver
            .handle(ReceiveEvent::Bytes(first))
            .expect("first frame is handled");
        let _ = driver
            .handle(ReceiveEvent::Persisted(persistence_id(&actions)))
            .expect("first persistence is handled");
    }

    fn assert_next_frame_aborts(driver: &mut ReceiveDriver, reason: AbortReason) {
        let second = encode_frame(2, b"L|1|N\r", FrameEnd::Final).expect("frame encodes");
        let actions = driver
            .handle(ReceiveEvent::Bytes(second))
            .expect("excess frame is handled");
        assert!(actions.contains(&Action::TransferAborted(reason)));
        assert!(
            actions
                .iter()
                .all(|action| !matches!(action, Action::PersistFrame { .. }))
        );
    }

    fn receive_persist_then_eot(text: &[u8]) -> Vec<Action> {
        let mut driver = ReceiveDriver::default();
        let _ = begin(&mut driver);
        let frame = encode_frame(1, text, FrameEnd::Intermediate).expect("frame encodes");
        let received = driver
            .handle(ReceiveEvent::Bytes(frame))
            .expect("frame is handled");
        let _ = driver
            .handle(ReceiveEvent::Persisted(persistence_id(&received)))
            .expect("frame persistence is handled");
        driver
            .handle(ReceiveEvent::Bytes(vec![EOT]))
            .expect("EOT is handled")
    }

    fn assert_incomplete_without_message(actions: &[Action]) {
        assert!(actions.contains(&Action::TransferAborted(AbortReason::IncompleteRecord)));
        assert!(
            actions
                .iter()
                .all(|action| !matches!(action, Action::MessageComplete(_)))
        );
    }

    #[test]
    fn req_int_009_ack_waits_for_matching_persistence() {
        let mut driver = ReceiveDriver::default();
        let _ = begin(&mut driver);
        let frame = encode_frame(1, b"H|\\^&\r", FrameEnd::Final).expect("frame encodes");
        let received = driver
            .handle(ReceiveEvent::Bytes(frame))
            .expect("frame is handled");
        assert!(
            received
                .iter()
                .all(|action| *action != Action::Send(ControlResponse::Ack))
        );

        let id = persistence_id(&received);
        let persisted = driver
            .handle(ReceiveEvent::Persisted(id))
            .expect("matching persistence is handled");
        assert!(persisted.contains(&Action::Send(ControlResponse::Ack)));
    }

    #[test]
    fn wrong_persistence_id_cannot_release_ack() {
        let mut driver = ReceiveDriver::default();
        let _ = begin(&mut driver);
        let frame = encode_frame(1, b"H|\\^&\r", FrameEnd::Final).expect("frame encodes");
        let received = driver
            .handle(ReceiveEvent::Bytes(frame))
            .expect("frame is handled");
        let id = persistence_id(&received);
        let wrong = FrameId(id.0 + 1);
        assert_eq!(
            driver.handle(ReceiveEvent::Persisted(wrong)),
            Err(DriverError::UnknownFrameId)
        );
        let persisted = driver
            .handle(ReceiveEvent::Persisted(id))
            .expect("matching persistence is handled");
        assert!(persisted.contains(&Action::Send(ControlResponse::Ack)));
    }

    #[test]
    fn req_int_003_duplicate_after_lost_ack_is_not_persisted_twice() {
        let mut driver = ReceiveDriver::default();
        let _ = begin(&mut driver);
        let frame = encode_frame(1, b"H|\\^&\r", FrameEnd::Final).expect("frame encodes");
        let first = driver
            .handle(ReceiveEvent::Bytes(frame.clone()))
            .expect("first frame is handled");
        let _ = driver
            .handle(ReceiveEvent::Persisted(persistence_id(&first)))
            .expect("frame persistence is handled");

        let duplicate = driver
            .handle(ReceiveEvent::Bytes(frame))
            .expect("duplicate is handled");
        assert!(duplicate.contains(&Action::Send(ControlResponse::Ack)));
        assert!(
            duplicate
                .iter()
                .all(|action| !matches!(action, Action::PersistFrame { .. }))
        );
    }

    #[test]
    fn buffered_duplicate_is_coalesced_before_sender_advances() {
        let mut driver = ReceiveDriver::default();
        let _ = begin(&mut driver);
        let frame = encode_frame(1, b"H|\\^&\r", FrameEnd::Final).expect("frame encodes");
        let first = driver
            .handle(ReceiveEvent::Bytes(frame.clone()))
            .expect("first frame is handled");
        let queued = driver
            .handle(ReceiveEvent::Bytes(frame))
            .expect("pending duplicate is buffered");
        assert!(queued.is_empty());

        let after_persist = driver
            .handle(ReceiveEvent::Persisted(persistence_id(&first)))
            .expect("frame persistence is handled");
        assert_eq!(
            after_persist
                .iter()
                .filter(|action| **action == Action::Send(ControlResponse::Ack))
                .count(),
            1
        );
        assert!(
            after_persist
                .iter()
                .all(|action| !matches!(action, Action::PersistFrame { .. }))
        );

        let second_frame =
            encode_frame(2, b"L|1|N\r", FrameEnd::Final).expect("second frame encodes");
        let second = driver
            .handle(ReceiveEvent::Bytes(second_frame))
            .expect("sender advances to second frame");
        assert!(
            second
                .iter()
                .all(|action| *action != Action::Send(ControlResponse::Ack))
        );
        let second_persisted = driver
            .handle(ReceiveEvent::Persisted(persistence_id(&second)))
            .expect("second persistence is handled");
        assert!(second_persisted.contains(&Action::Send(ControlResponse::Ack)));
    }

    #[test]
    fn eot_with_incomplete_record_aborts_without_message() {
        let actions = receive_persist_then_eot(b"H|\\^&");
        assert_incomplete_without_message(&actions);
    }

    #[test]
    fn etb_with_record_terminator_cannot_complete_on_eot() {
        let actions = receive_persist_then_eot(b"H|\\^&\r");
        assert_incomplete_without_message(&actions);
    }

    #[test]
    fn timeout_while_persistence_is_pending_aborts_without_frame_ack() {
        let mut driver = ReceiveDriver::default();
        let _ = begin(&mut driver);
        let frame = encode_frame(1, b"H|\\^&\r", FrameEnd::Final).expect("frame encodes");
        let received = driver
            .handle(ReceiveEvent::Bytes(frame))
            .expect("frame is handled");
        let timer_id = received
            .iter()
            .find_map(|action| match action {
                Action::StartTimer { timer_id, .. } => Some(*timer_id),
                Action::Send(_)
                | Action::PersistFrame { .. }
                | Action::MessageComplete(_)
                | Action::TransferAborted(_)
                | Action::StopTimer => None,
            })
            .expect("persistence timer exists");
        let timed_out = driver
            .handle(ReceiveEvent::TimerExpired(timer_id))
            .expect("timeout is handled");
        assert!(timed_out.contains(&Action::TransferAborted(AbortReason::ReceiveTimeout)));
        assert!(
            timed_out
                .iter()
                .all(|action| *action != Action::Send(ControlResponse::Ack))
        );
    }

    #[test]
    fn message_limit_aborts_before_persistence_or_ack() {
        let mut driver = ReceiveDriver::new(3);
        let _ = begin(&mut driver);
        let frame = encode_frame(1, b"H|X\r", FrameEnd::Final).expect("frame encodes");
        let actions = driver
            .handle(ReceiveEvent::Bytes(frame))
            .expect("oversize message is handled");
        assert!(actions.contains(&Action::TransferAborted(AbortReason::MessageTooLarge)));
        assert!(actions.iter().all(|action| {
            !matches!(
                action,
                Action::PersistFrame { .. } | Action::Send(ControlResponse::Ack)
            )
        }));
    }

    #[test]
    fn empty_frame_aborts_before_persistence_or_ack() {
        let mut driver = ReceiveDriver::default();
        let _ = begin(&mut driver);
        let frame = encode_frame(1, b"", FrameEnd::Final).expect("empty frame encodes");
        let actions = driver
            .handle(ReceiveEvent::Bytes(frame))
            .expect("empty frame is handled");
        assert!(actions.contains(&Action::TransferAborted(AbortReason::EmptyFrame)));
        assert!(actions.iter().all(|action| {
            !matches!(
                action,
                Action::PersistFrame { .. } | Action::Send(ControlResponse::Ack)
            )
        }));
    }

    #[test]
    fn frame_count_limit_aborts_before_excess_frame_persistence() {
        let mut driver = ReceiveDriver {
            max_frames: 1,
            ..ReceiveDriver::default()
        };
        let first = encode_frame(1, b"H|x\r", FrameEnd::Final).expect("frame encodes");
        persist_first_frame(&mut driver, first);
        assert_next_frame_aborts(&mut driver, AbortReason::FrameLimitExceeded);
    }

    #[test]
    fn wire_byte_limit_aborts_before_excess_frame_persistence() {
        let first = encode_frame(1, b"H|x\r", FrameEnd::Final).expect("frame encodes");
        let mut driver = ReceiveDriver {
            max_wire_bytes: first.len(),
            ..ReceiveDriver::default()
        };
        persist_first_frame(&mut driver, first);
        assert_next_frame_aborts(&mut driver, AbortReason::WireBytesExceeded);
    }

    #[test]
    fn buffered_wire_input_is_bounded_while_persistence_is_pending() {
        let mut driver = ReceiveDriver::new(3);
        let _ = begin(&mut driver);
        let frame = encode_frame(1, b"H\r", FrameEnd::Final).expect("frame encodes");
        let received = driver
            .handle(ReceiveEvent::Bytes(frame))
            .expect("frame is handled");
        assert!(
            received
                .iter()
                .any(|action| matches!(action, Action::PersistFrame { .. }))
        );

        let overflow = driver
            .handle(ReceiveEvent::Bytes(vec![b'A'; MAX_WIRE_FRAME_BYTES + 1]))
            .expect("buffer overflow is handled");
        assert!(overflow.contains(&Action::TransferAborted(AbortReason::InputBufferExceeded)));
        assert!(
            overflow
                .iter()
                .all(|action| *action != Action::Send(ControlResponse::Ack))
        );
    }

    #[test]
    fn sixth_bad_frame_aborts_after_nak() {
        let mut driver = ReceiveDriver::default();
        let _ = begin(&mut driver);
        let mut bad = encode_frame(1, b"H|\\^&\r", FrameEnd::Final).expect("frame encodes");
        let checksum_index = bad.len() - 4;
        bad[checksum_index] = if bad[checksum_index] == b'0' {
            b'1'
        } else {
            b'0'
        };

        for _ in 0..5 {
            let actions = driver
                .handle(ReceiveEvent::Bytes(bad.clone()))
                .expect("bad frame is handled");
            assert!(actions.contains(&Action::Send(ControlResponse::Nak)));
            assert!(
                actions
                    .iter()
                    .all(|action| !matches!(action, Action::TransferAborted(_)))
            );
        }
        let sixth = driver
            .handle(ReceiveEvent::Bytes(bad))
            .expect("sixth bad frame is handled");
        assert!(sixth.contains(&Action::Send(ControlResponse::Nak)));
        assert!(sixth.contains(&Action::TransferAborted(AbortReason::RetryLimitExceeded)));
    }

    #[test]
    fn complete_transfer_returns_original_records() {
        let records = vec![b"H|\\^&|||SYNTH-ANALYZER^1.0".to_vec(), b"L|1|N".to_vec()];
        let frames = encode_records(&records).expect("records encode");
        let mut driver = ReceiveDriver::default();
        let _ = begin(&mut driver);
        for frame in frames {
            let actions = driver
                .handle(ReceiveEvent::Bytes(frame))
                .expect("frame is handled");
            let _ = driver
                .handle(ReceiveEvent::Persisted(persistence_id(&actions)))
                .expect("frame persistence is handled");
        }
        let complete = driver
            .handle(ReceiveEvent::Bytes(vec![EOT]))
            .expect("EOT is handled");
        assert!(complete.contains(&Action::MessageComplete(records)));
    }

    #[test]
    fn packed_records_in_one_frame_are_emitted_individually() {
        let records = vec![b"H|\\^&|||SYNTHETIC".to_vec(), b"L|1|N".to_vec()];
        let mut packed = records[0].clone();
        packed.push(CR);
        packed.extend_from_slice(&records[1]);
        packed.push(CR);
        let frame = encode_frame(1, &packed, FrameEnd::Final).expect("packed frame encodes");
        let mut driver = ReceiveDriver::default();
        let _ = begin(&mut driver);
        let actions = driver
            .handle(ReceiveEvent::Bytes(frame))
            .expect("packed frame is handled");
        let _ = driver
            .handle(ReceiveEvent::Persisted(persistence_id(&actions)))
            .expect("packed frame persistence is handled");
        let complete = driver
            .handle(ReceiveEvent::Bytes(vec![EOT]))
            .expect("EOT is handled");
        assert!(complete.contains(&Action::MessageComplete(records)));
    }

    proptest! {
        #[test]
        fn arbitrary_byte_chunks_and_timer_events_never_panic(
            chunks in prop::collection::vec(prop::collection::vec(any::<u8>(), 0..80), 0..40),
        ) {
            let mut driver = ReceiveDriver::new(4096);
            for chunk in chunks {
                let _ = driver.handle(ReceiveEvent::Bytes(chunk));
                let _ = driver.handle(ReceiveEvent::TimerExpired(TimerId(0)));
                let _ = driver.handle(ReceiveEvent::Persisted(FrameId(0)));
            }
        }
    }
}
