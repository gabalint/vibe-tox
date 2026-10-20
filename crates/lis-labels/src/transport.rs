//! Raw label transports: TCP (port 9100) and a CUPS raw queue.
//!
//! A successful print means the bytes were handed to the printer or spooler,
//! not that a label came out (AGENTS.md: a transport acknowledgement is not
//! proof of processing).
//!
//! Labels carry patient names and birth dates, so a network printer must sit
//! on the laboratory's own network: [`check_destination`] allows private and
//! link-local addresses only, and is applied both when a printer is
//! configured and before every TCP print (NFR-32). The engine's
//! [`PrinterDestinations`] policy also admits loopback in tests only, where a
//! local listener stands in for the printer.

use std::io::{self, Write};
use std::net::{IpAddr, Shutdown, SocketAddr, TcpStream};
use std::path::Path;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant};

/// The conventional raw printing port of network label printers.
pub const DEFAULT_TCP_PORT: u16 = 9100;

/// Longest CUPS queue name CUPS accepts.
const MAX_QUEUE_NAME: usize = 127;

/// Where `lp` is looked for, in order. Never a `PATH` lookup, so an `lp`
/// planted earlier on the app's `PATH` cannot receive labels. Both are
/// root-owned system directories: macOS and the mainstream Linux CUPS
/// packages install `/usr/bin/lp`; `/bin/lp` covers systems without a merged
/// `/usr`. User-writable prefixes (`/usr/local`, `/opt/homebrew`) are left out.
const LP_PATHS: [&str; 2] = ["/usr/bin/lp", "/bin/lp"];

/// How long `lp` may take to accept a job before it is killed. Handing a
/// small label to the local scheduler takes well under a second.
const SPOOLER_DEADLINE: Duration = Duration::from_secs(30);

/// How often a running `lp` is checked for exit.
const SPOOLER_POLL: Duration = Duration::from_millis(20);

/// Where label bytes go.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Transport {
    /// Raw TCP to a network printer, usually on [`DEFAULT_TCP_PORT`]. An IP
    /// address, not a host name, so no DNS lookup can stall printing or
    /// redirect it; [`Transport::print`] refuses it unless it passes
    /// [`check_destination`].
    Tcp {
        address: SocketAddr,
        connect_timeout: Duration,
        write_timeout: Duration,
    },
    /// A CUPS queue configured for raw data, via `lp -d <queue> -o raw`.
    /// Not available on Windows in M0.
    Cups { queue: QueueName },
}

/// A CUPS queue name restricted to ASCII letters, digits, `-`, `_` and `.`,
/// not starting with `-`, so it can never be read as an `lp` option.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueueName(String);

impl QueueName {
    /// Validates a queue name.
    pub fn new(name: &str) -> Result<Self, PrintError> {
        let allowed = |c: char| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.');
        let valid = !name.is_empty()
            && name.len() <= MAX_QUEUE_NAME
            && !name.starts_with('-')
            && name.chars().all(allowed);
        if valid {
            Ok(Self(name.to_owned()))
        } else {
            Err(PrintError::InvalidQueueName)
        }
    }
}

/// Why label bytes could not be handed over. Never contains label content.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PrintError {
    /// The queue name failed validation.
    #[error("invalid printer queue name")]
    InvalidQueueName,
    /// The transport is not supported on this operating system.
    #[error("this printer transport is not supported on this operating system")]
    UnsupportedTransport,
    /// Nothing listens at the printer address.
    #[error("printer refused the connection")]
    ConnectionRefused,
    /// The printer did not accept the connection in time.
    #[error("printer connection timed out")]
    ConnectTimeout,
    /// The printer stopped accepting data in time.
    #[error("printer write timed out")]
    WriteTimeout,
    /// The printer address is not a private or link-local address on the
    /// laboratory's network.
    #[error("printer address is not on the laboratory network")]
    DestinationRefused,
    /// Another network failure.
    #[error("printer connection failed: {kind}")]
    Network { kind: io::ErrorKind },
    /// The `lp` command could not be started.
    #[error("print spooler command unavailable: {kind}")]
    SpoolerUnavailable { kind: io::ErrorKind },
    /// `lp` exited with a failure status.
    #[error("print spooler rejected the job (exit code {code:?})")]
    SpoolerRejected { code: Option<i32> },
    /// `lp` did not finish in time and was stopped.
    #[error("print spooler did not respond in time")]
    SpoolerTimeout,
}

/// Which printer addresses may receive labels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrinterDestinations {
    /// Private and link-local addresses only: the app's only policy.
    LabNetwork,
    /// The laboratory network plus loopback, so scenario tests can print to a
    /// local test printer. Never used by the app.
    LabNetworkAndLoopback,
}

/// Allows a network printer address only if it is private or link-local:
/// IPv4 10/8, 172.16/12, 192.168/16 and 169.254/16; IPv6 fc00::/7 and
/// fe80::/10; or an IPv4-mapped IPv6 address of an allowed IPv4 address.
/// Everything else is refused, including loopback, unspecified, multicast,
/// broadcast, shared, documentation and public addresses, so label content
/// cannot be sent off the laboratory network or to another local program.
/// Under [`PrinterDestinations::LabNetworkAndLoopback`] loopback is allowed too.
pub fn check_destination(ip: IpAddr, destinations: PrinterDestinations) -> Result<(), PrintError> {
    let lab_network = match ip {
        IpAddr::V4(v4) => v4.is_private() || v4.is_link_local(),
        IpAddr::V6(v6) => match v6.to_ipv4_mapped() {
            Some(v4) => v4.is_private() || v4.is_link_local(),
            None => v6.is_unique_local() || v6.is_unicast_link_local(),
        },
    };
    let loopback = destinations == PrinterDestinations::LabNetworkAndLoopback
        && ip.to_canonical().is_loopback();
    if lab_network || loopback {
        Ok(())
    } else {
        Err(PrintError::DestinationRefused)
    }
}

impl Transport {
    /// Raw TCP on port 9100 with default timeouts (5 s connect, 10 s write).
    pub fn tcp(ip: std::net::IpAddr) -> Self {
        Self::Tcp {
            address: SocketAddr::new(ip, DEFAULT_TCP_PORT),
            connect_timeout: Duration::from_secs(5),
            write_timeout: Duration::from_secs(10),
        }
    }

    /// Hands the exact label bytes to the printer or spooler. A TCP
    /// address is checked again here, so a printer configured before the
    /// destination rule existed cannot receive labels either.
    pub fn print(&self, label: &[u8], destinations: PrinterDestinations) -> Result<(), PrintError> {
        match self {
            Self::Tcp {
                address,
                connect_timeout,
                write_timeout,
            } => {
                check_destination(address.ip(), destinations)?;
                print_tcp(*address, *connect_timeout, *write_timeout, label)
            }
            Self::Cups { queue } => print_cups(queue, label),
        }
    }
}

fn print_tcp(
    address: SocketAddr,
    connect_timeout: Duration,
    write_timeout: Duration,
    label: &[u8],
) -> Result<(), PrintError> {
    let mut stream = TcpStream::connect_timeout(&address, connect_timeout)
        .map_err(|error| connect_error(error.kind()))?;
    stream
        .set_write_timeout(Some(write_timeout))
        .map_err(|error| write_error(error.kind()))?;
    stream
        .write_all(label)
        .map_err(|error| write_error(error.kind()))?;
    stream.flush().map_err(|error| write_error(error.kind()))?;
    // Closing the write side tells the printer the job is complete.
    stream
        .shutdown(Shutdown::Write)
        .map_err(|error| write_error(error.kind()))
}

fn connect_error(kind: io::ErrorKind) -> PrintError {
    match kind {
        io::ErrorKind::ConnectionRefused => PrintError::ConnectionRefused,
        io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock => PrintError::ConnectTimeout,
        other => PrintError::Network { kind: other },
    }
}

fn write_error(kind: io::ErrorKind) -> PrintError {
    match kind {
        // Unix reports an expired socket timeout as `WouldBlock`.
        io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock => PrintError::WriteTimeout,
        other => PrintError::Network { kind: other },
    }
}

fn print_cups(queue: &QueueName, label: &[u8]) -> Result<(), PrintError> {
    if cfg!(windows) {
        return Err(PrintError::UnsupportedTransport);
    }
    let lp = LP_PATHS
        .iter()
        .map(Path::new)
        .find(|path| path.is_file())
        .ok_or(PrintError::SpoolerUnavailable {
            kind: io::ErrorKind::NotFound,
        })?;
    spool(lp_command(lp, queue), label, SPOOLER_DEADLINE)
}

/// `lp` run directly, without a shell; its output is discarded because it can
/// echo job details, which also means no output pipe can fill and block it.
fn lp_command(lp: &Path, queue: &QueueName) -> Command {
    let mut command = Command::new(lp);
    command
        .args(["-d", &queue.0, "-o", "raw"])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    command
}

/// Runs the spooler with the label on stdin. A writer thread feeds stdin so
/// a spooler that stops reading cannot block this thread past `deadline`;
/// after the deadline the spooler is killed and reaped.
fn spool(mut command: Command, label: &[u8], deadline: Duration) -> Result<(), PrintError> {
    let spooler_error = |error: io::Error| PrintError::SpoolerUnavailable { kind: error.kind() };
    let mut child = command.spawn().map_err(spooler_error)?;
    let stdin = child.stdin.take();
    let bytes = label.to_vec();
    // Dropping stdin at the end of the thread closes it, which ends the job.
    let writer = match thread::Builder::new()
        .spawn(move || stdin.map_or(Ok(()), |mut stdin| stdin.write_all(&bytes)))
    {
        Ok(writer) => writer,
        Err(error) => {
            stop(&mut child).map_err(spooler_error)?;
            return Err(spooler_error(error));
        }
    };
    let status = wait_until(&mut child, deadline).map_err(spooler_error)?;
    let Some(status) = status else {
        // The writer ends once the killed spooler's end of the pipe closes.
        return Err(PrintError::SpoolerTimeout);
    };
    let written = writer
        .join()
        .unwrap_or_else(|_| Err(io::Error::other("stdin writer panicked")));
    if !status.success() {
        return Err(PrintError::SpoolerRejected {
            code: status.code(),
        });
    }
    written.map_err(spooler_error)
}

/// The child's exit status, or `None` after killing and reaping it because
/// it outlived `deadline`.
fn wait_until(child: &mut Child, deadline: Duration) -> io::Result<Option<ExitStatus>> {
    let started = Instant::now();
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(Some(status));
        }
        if started.elapsed() >= deadline {
            stop(child)?;
            return Ok(None);
        }
        thread::sleep(SPOOLER_POLL);
    }
}

/// Kills and reaps a child that has not been waited for. Not reaped yet, so
/// the kill succeeds even if it just exited; the wait leaves no zombie.
fn stop(child: &mut Child) -> io::Result<()> {
    child.kill()?;
    child.wait().map(drop)
}

#[cfg(test)]
mod tests {
    use std::io::Read;
    use std::net::{Ipv4Addr, Ipv6Addr, TcpListener};

    use super::*;

    /// Sends to a loopback test printer under the test destination policy. The connect
    /// timeout outlasts Windows, which retries a refused loopback connection for about
    /// two seconds before reporting the refusal.
    fn print_local(port: u16, write_timeout: Duration, label: &[u8]) -> Result<(), PrintError> {
        let transport = Transport::Tcp {
            address: SocketAddr::new(Ipv4Addr::LOCALHOST.into(), port),
            connect_timeout: Duration::from_secs(5),
            write_timeout,
        };
        transport.print(label, PrinterDestinations::LabNetworkAndLoopback)
    }

    fn ip(text: &str) -> IpAddr {
        text.parse().expect("IP literal")
    }

    #[test]
    fn tcp_print_delivers_exact_bytes() {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).expect("bind");
        let port = listener.local_addr().expect("address").port();
        let receiver = thread::spawn(move || {
            let (mut connection, _) = listener.accept().expect("accept");
            let mut received = Vec::new();
            connection.read_to_end(&mut received).expect("read");
            received
        });
        let label = crate::render(
            &crate::LabelTemplateRef {
                id: "specimen".to_owned(),
                version: 1,
            },
            &[
                ("container_id", "C-SYN-0001"),
                ("subject_name", "SYNTHETIC, Patient Ëta"),
                ("birth_date", "1980-01-01"),
                ("accession", "ACC-SYN-0001"),
                ("matrix", "Urine"),
            ],
        )
        .expect("renders");
        print_local(port, Duration::from_secs(2), &label).expect("prints");
        assert_eq!(receiver.join().expect("receiver"), label);
    }

    #[test]
    fn tcp_print_reports_connection_refused() {
        let port = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
            .and_then(|listener| listener.local_addr())
            .expect("ephemeral port")
            .port();
        // The listener is dropped, so nothing listens on the port any more.
        assert_eq!(
            print_local(port, Duration::from_secs(1), b"^XA^XZ"),
            Err(PrintError::ConnectionRefused)
        );
    }

    // Windows loopback accepted the whole 64 MiB write into its buffers, so a stalled
    // reader cannot be simulated this way there.
    #[cfg(unix)]
    #[test]
    fn tcp_print_times_out_when_printer_stops_reading() {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).expect("bind");
        let port = listener.local_addr().expect("address").port();
        // Accept but never read, so the socket buffers fill and the write stalls.
        let holder = thread::spawn(move || listener.accept().map(|(connection, _)| connection));
        let oversized = vec![b'A'; 64 * 1024 * 1024];
        let result = print_local(port, Duration::from_millis(200), &oversized);
        assert_eq!(result, Err(PrintError::WriteTimeout));
        drop(holder.join());
    }

    #[test]
    fn connect_timeouts_map_to_connect_timeout() {
        assert_eq!(
            connect_error(io::ErrorKind::TimedOut),
            PrintError::ConnectTimeout
        );
        assert_eq!(
            connect_error(io::ErrorKind::WouldBlock),
            PrintError::ConnectTimeout
        );
    }

    #[test]
    fn default_tcp_transport_uses_port_9100() {
        let Transport::Tcp { address, .. } = Transport::tcp(Ipv4Addr::new(192, 168, 2, 10).into())
        else {
            panic!("tcp transport");
        };
        assert_eq!(address.port(), 9100);
    }

    #[test]
    fn private_and_link_local_destinations_are_allowed() {
        for allowed in [
            "10.0.0.1",
            "10.255.255.254",
            "172.16.0.1",
            "172.31.255.254",
            "192.168.1.20",
            "169.254.10.20",
            "fc00::1",
            "fd12:3456:789a::20",
            "fe80::1",
            "febf::1",
            "::ffff:192.168.1.20",
            "::ffff:169.254.1.1",
        ] {
            for destinations in [
                PrinterDestinations::LabNetwork,
                PrinterDestinations::LabNetworkAndLoopback,
            ] {
                assert_eq!(
                    check_destination(ip(allowed), destinations),
                    Ok(()),
                    "{allowed}"
                );
            }
        }
    }

    #[test]
    fn every_other_destination_class_is_refused() {
        for refused in [
            // Loopback and unspecified.
            "127.0.0.1",
            "127.1.2.3",
            "::1",
            "0.0.0.0",
            "::",
            // Multicast and broadcast.
            "224.0.0.1",
            "239.255.255.250",
            "ff02::1",
            "255.255.255.255",
            // Just outside the private ranges.
            "172.15.255.255",
            "172.32.0.1",
            "192.169.0.1",
            "169.255.0.1",
            "fe00::1",
            "fec0::1",
            // Public.
            "8.8.8.8",
            "2001:4860:4860::8888",
            // Shared (carrier-grade NAT), benchmarking and documentation.
            "100.64.0.1",
            "198.18.0.1",
            "192.0.2.10",
            "198.51.100.10",
            "203.0.113.10",
            "2001:db8::10",
            // IPv4 embedded in IPv6 other than as an allowed mapped address.
            "::ffff:8.8.8.8",
            "::ffff:127.0.0.1",
            "::ffff:0.0.0.0",
            "::192.168.1.20",
            "64:ff9b::c0a8:114",
        ] {
            assert_eq!(
                check_destination(ip(refused), PrinterDestinations::LabNetwork),
                Err(PrintError::DestinationRefused),
                "{refused}"
            );
        }
    }

    #[test]
    fn the_test_policy_adds_loopback_and_nothing_else() {
        for loopback in ["127.0.0.1", "127.1.2.3", "::1", "::ffff:127.0.0.1"] {
            let loopback = ip(loopback);
            let test = check_destination(loopback, PrinterDestinations::LabNetworkAndLoopback);
            assert_eq!(test, Ok(()), "{loopback}");
        }
        for refused in ["0.0.0.0", "::", "8.8.8.8", "224.0.0.1", "100.64.0.1"] {
            assert_eq!(
                check_destination(ip(refused), PrinterDestinations::LabNetworkAndLoopback),
                Err(PrintError::DestinationRefused),
                "{refused}"
            );
        }
    }

    #[test]
    fn tcp_print_refuses_a_disallowed_destination_before_connecting() {
        // Only the destination check yields `DestinationRefused`; no listener
        // is bound, so this test cannot take a port another test relies on.
        for address in [
            SocketAddr::new(Ipv4Addr::LOCALHOST.into(), DEFAULT_TCP_PORT),
            SocketAddr::new(Ipv6Addr::LOCALHOST.into(), DEFAULT_TCP_PORT),
            SocketAddr::new(Ipv4Addr::new(8, 8, 8, 8).into(), DEFAULT_TCP_PORT),
        ] {
            let transport = Transport::Tcp {
                address,
                connect_timeout: Duration::from_secs(1),
                write_timeout: Duration::from_secs(1),
            };
            assert_eq!(
                transport.print(b"^XA^XZ", PrinterDestinations::LabNetwork),
                Err(PrintError::DestinationRefused),
                "{address}"
            );
        }
    }

    #[test]
    fn queue_names_are_validated() {
        for valid in ["Zebra_GK420d", "lab-labels.1", "a"] {
            assert!(QueueName::new(valid).is_ok(), "{valid}");
        }
        let too_long = "q".repeat(MAX_QUEUE_NAME + 1);
        for invalid in [
            "",
            "-o",
            "two words",
            "q;rm",
            "q/x",
            "q\n",
            "quëue",
            too_long.as_str(),
        ] {
            assert_eq!(
                QueueName::new(invalid),
                Err(PrintError::InvalidQueueName),
                "{invalid:?}"
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn cups_command_is_lp_by_absolute_path_raw_without_a_shell() {
        use std::ffi::OsStr;

        assert!(LP_PATHS.iter().all(|path| Path::new(path).is_absolute()));
        let lp = Path::new(LP_PATHS[0]);
        let command = lp_command(lp, &QueueName::new("lab-labels").expect("valid"));
        assert_eq!(command.get_program(), lp.as_os_str());
        let args: Vec<&OsStr> = command.get_args().collect();
        assert_eq!(args, ["-d", "lab-labels", "-o", "raw"]);
    }

    #[cfg(unix)]
    fn shell(script: &str) -> Command {
        let mut command = Command::new("/bin/sh");
        command
            .args(["-c", script])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        command
    }

    #[cfg(unix)]
    #[test]
    fn a_spooler_that_reads_the_label_and_exits_zero_succeeds() {
        let label = vec![b'A'; 1024 * 1024];
        assert_eq!(
            spool(shell("cat >/dev/null"), &label, Duration::from_secs(10)),
            Ok(())
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_failing_spooler_is_rejected_with_its_exit_code() {
        assert_eq!(
            spool(
                shell("cat >/dev/null; exit 3"),
                b"^XA^XZ",
                Duration::from_secs(10)
            ),
            Err(PrintError::SpoolerRejected { code: Some(3) })
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_spooler_that_never_reads_or_exits_is_killed_at_the_deadline() {
        // Larger than any pipe buffer, so writing stdin would block forever.
        let label = vec![b'A'; 4 * 1024 * 1024];
        let started = Instant::now();
        let result = spool(shell("exec sleep 30"), &label, Duration::from_millis(200));
        assert_eq!(result, Err(PrintError::SpoolerTimeout));
        assert!(started.elapsed() < Duration::from_secs(10));
    }

    #[cfg(unix)]
    #[test]
    fn a_timed_out_spooler_is_reaped() {
        let mut child = shell("exec sleep 30").spawn().expect("spawn");
        let pid = child.id().to_string();
        assert!(
            wait_until(&mut child, Duration::from_millis(100))
                .expect("wait")
                .is_none()
        );
        // A zombie still answers `kill -0`; a reaped process does not.
        let probe = Command::new("/bin/sh")
            .args(["-c", "kill -0 \"$0\" 2>/dev/null", &pid])
            .status()
            .expect("probe");
        assert!(!probe.success());
    }

    #[cfg(unix)]
    #[test]
    fn a_missing_spooler_program_is_unavailable() {
        let mut command = Command::new("/nonexistent/synthetic-lp");
        command.stdin(Stdio::piped());
        assert_eq!(
            spool(command, b"^XA^XZ", Duration::from_secs(1)),
            Err(PrintError::SpoolerUnavailable {
                kind: io::ErrorKind::NotFound
            })
        );
    }

    #[cfg(windows)]
    #[test]
    fn cups_is_unsupported_on_windows() {
        let transport = Transport::Cups {
            queue: QueueName::new("labels").expect("valid"),
        };
        assert_eq!(
            transport.print(b"^XA^XZ", PrinterDestinations::LabNetwork),
            Err(PrintError::UnsupportedTransport)
        );
    }
}
