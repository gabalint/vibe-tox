//! Serial line setup shared by AU and ASTM listeners.
use crate::listener::{IO_POLL, StatusMap, detail_for_error, set_status};
use crate::session::{StreamContext, run_protocol_stream};
use crate::{
    AuConnectionSettings, DataBits, ListenerDetailCode, ListenerErrorCode, ListenerState, Parity,
    StopBits,
};
use lis_core::InstrumentId;
use std::sync::atomic::{AtomicBool, Ordering};

pub(crate) fn open(
    path: String,
    baud: u32,
    bits: DataBits,
    parity: Parity,
    stop: StopBits,
) -> serialport::Result<Box<dyn serialport::SerialPort>> {
    serialport::new(path, baud)
        .data_bits(serial_data_bits(bits))
        .parity(serial_parity(parity))
        .stop_bits(serial_stop_bits(stop))
        .flow_control(serialport::FlowControl::None)
        .timeout(IO_POLL)
        .open()
}

pub(crate) fn run(
    port: &mut dyn serialport::SerialPort,
    context: &StreamContext<'_>,
    au: Option<&AuConnectionSettings>,
) {
    set_status(
        context.statuses,
        context.instrument_id,
        ListenerState::Connected,
        None,
    );
    tracing::info!(instrument_id = %context.instrument_id, transport = context.transport.as_str(), "listener started");
    let result = run_protocol_stream(port, context, au);
    finish(
        result,
        context.statuses,
        context.instrument_id,
        context.stop,
    );
}

fn finish(
    result: Result<(), ListenerErrorCode>,
    statuses: &StatusMap,
    instrument_id: InstrumentId,
    stop: &AtomicBool,
) {
    if stop.load(Ordering::Acquire) && result.is_ok() {
        set_status(
            statuses,
            instrument_id,
            ListenerState::Disabled,
            Some(ListenerDetailCode::ListenerStopped),
        );
        tracing::info!(instrument_id = %instrument_id, "listener stopped");
        return;
    }
    let code = match result {
        Err(code) => code,
        Ok(()) => ListenerErrorCode::TransportFailed,
    };
    set_status(
        statuses,
        instrument_id,
        ListenerState::Error,
        Some(detail_for_error(code)),
    );
    tracing::warn!(instrument_id = %instrument_id, error_code = ?code, "listener stopped");
}

const fn serial_data_bits(value: DataBits) -> serialport::DataBits {
    match value {
        DataBits::Five => serialport::DataBits::Five,
        DataBits::Six => serialport::DataBits::Six,
        DataBits::Seven => serialport::DataBits::Seven,
        DataBits::Eight => serialport::DataBits::Eight,
    }
}

const fn serial_parity(value: Parity) -> serialport::Parity {
    match value {
        Parity::None => serialport::Parity::None,
        Parity::Odd => serialport::Parity::Odd,
        Parity::Even => serialport::Parity::Even,
    }
}

const fn serial_stop_bits(value: StopBits) -> serialport::StopBits {
    match value {
        StopBits::One => serialport::StopBits::One,
        StopBits::Two => serialport::StopBits::Two,
    }
}
