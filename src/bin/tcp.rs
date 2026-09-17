use std::io::{self, BufReader, BufWriter, ErrorKind, Read, Write};
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream};
use std::process::exit;
use std::thread;
use std::time::{Duration, Instant};

use fomoxa_codec_bench::cli;
use fomoxa_codec_bench::format::{self, Encoder, Format, Sample};
use fomoxa_codec_bench::measure::{nanos, percentile, thousands, thread_cpu_seconds};
use fomoxa_codec_bench::pair::Pair;
use fomoxa_codec_bench::samples::{self, Visitor};

const HEADER_LEN: usize = 4;
const STREAM_BUFFER: usize = 64 * 1024;
const CHECK_EVERY: u32 = 64;
const MIB: f64 = 1024.0 * 1024.0;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Buffered,
    Unbuffered,
    Echo,
}

impl Mode {
    const ALL: [Mode; 3] = [Mode::Buffered, Mode::Unbuffered, Mode::Echo];

    fn name(self) -> &'static str {
        match self {
            Mode::Buffered => "buffered",
            Mode::Unbuffered => "unbuffered",
            Mode::Echo => "echo",
        }
    }
}

#[derive(Default)]
struct Side {
    messages: u64,
    bytes: u64,
    failures: u64,
    elapsed: Duration,
    cpu_seconds: f64,
}

impl Side {
    fn cpu_nanos_per_message(&self) -> f64 {
        self.cpu_seconds * 1e9 / self.messages.max(1) as f64
    }

    fn cores(&self) -> f64 {
        self.cpu_seconds / self.elapsed.as_secs_f64().max(f64::EPSILON)
    }
}

fn frame(payload: &[u8], out: &mut Vec<u8>) {
    out.clear();
    out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    out.extend_from_slice(payload);
}

fn read_frame(reader: &mut impl Read, payload: &mut Vec<u8>) -> io::Result<bool> {
    let mut header = [0u8; HEADER_LEN];
    match reader.read_exact(&mut header) {
        Ok(()) => {}
        Err(error) if error.kind() == ErrorKind::UnexpectedEof => return Ok(false),
        Err(error) => return Err(error),
    }
    payload.resize(u32::from_le_bytes(header) as usize, 0);
    reader.read_exact(payload)?;
    Ok(true)
}

fn fail(context: &str, error: io::Error) -> ! {
    eprintln!("tcp: {context}: {error}");
    exit(1);
}

fn listen() -> (TcpListener, SocketAddr) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap_or_else(|error| fail("bind", error));
    let address = listener.local_addr().unwrap_or_else(|error| fail("local address", error));
    (listener, address)
}

fn accept(listener: &TcpListener) -> TcpStream {
    let (stream, _) = listener.accept().unwrap_or_else(|error| fail("accept", error));
    let _ = stream.set_nodelay(true);
    stream
}

fn connect(address: SocketAddr) -> TcpStream {
    let stream = TcpStream::connect(address).unwrap_or_else(|error| fail("connect", error));
    let _ = stream.set_nodelay(true);
    stream
}

fn stream<M: Pair>(sample: &Sample<M>, format: Format, mode: Mode, duration: Duration) -> (Side, Side) {
    let (listener, address) = listen();
    thread::scope(|scope| {
        let receiver = scope.spawn(|| {
            let stream = accept(&listener);
            let cpu_started = thread_cpu_seconds();
            let started = Instant::now();
            let mut reader = BufReader::with_capacity(STREAM_BUFFER, stream);
            let mut payload = Vec::new();
            let mut side = Side::default();
            while read_frame(&mut reader, &mut payload).unwrap_or_else(|error| fail("read", error)) {
                match format::decode::<M>(format, &payload) {
                    Some(_) => side.messages += 1,
                    None => side.failures += 1,
                }
                side.bytes += (HEADER_LEN + payload.len()) as u64;
            }
            side.elapsed = started.elapsed();
            side.cpu_seconds = thread_cpu_seconds() - cpu_started;
            side
        });

        let capacity = if mode == Mode::Buffered { STREAM_BUFFER } else { 0 };
        let mut writer = BufWriter::with_capacity(capacity, connect(address));
        let mut encoder = Encoder::default();
        let mut buffer = Vec::new();
        let cpu_started = thread_cpu_seconds();
        let started = Instant::now();
        let mut sender = Side::default();
        while started.elapsed() < duration {
            for _ in 0..CHECK_EVERY {
                frame(encoder.encode(format, sample), &mut buffer);
                writer.write_all(&buffer).unwrap_or_else(|error| fail("write", error));
                sender.messages += 1;
                sender.bytes += buffer.len() as u64;
            }
        }
        let stream = writer
            .into_inner()
            .unwrap_or_else(|error| fail("flush", error.into_error()));
        sender.elapsed = started.elapsed();
        sender.cpu_seconds = thread_cpu_seconds() - cpu_started;
        let _ = stream.shutdown(Shutdown::Write);

        let receiver = receiver.join().unwrap_or_else(|_| {
            eprintln!("tcp: the receiver thread panicked");
            exit(1);
        });
        (sender, receiver)
    })
}

fn echo<M: Pair>(sample: &Sample<M>, format: Format, duration: Duration) -> (Side, Side, Vec<u32>) {
    let (listener, address) = listen();
    thread::scope(|scope| {
        let server = scope.spawn(|| {
            let mut stream = accept(&listener);
            let cpu_started = thread_cpu_seconds();
            let started = Instant::now();
            let mut encoder = Encoder::default();
            let mut payload = Vec::new();
            let mut reply = Vec::new();
            let mut side = Side::default();
            while read_frame(&mut stream, &mut payload).unwrap_or_else(|error| fail("server read", error)) {
                let Some(decoded) = format::decode::<M>(format, &payload) else {
                    side.failures += 1;
                    continue;
                };
                frame(encoder.encode_decoded(&decoded), &mut reply);
                stream.write_all(&reply).unwrap_or_else(|error| fail("server write", error));
                side.messages += 1;
                side.bytes += reply.len() as u64;
            }
            side.elapsed = started.elapsed();
            side.cpu_seconds = thread_cpu_seconds() - cpu_started;
            side
        });

        let mut stream = connect(address);
        let mut encoder = Encoder::default();
        let mut request = Vec::new();
        let mut payload = Vec::new();
        let mut round_trips = Vec::new();
        let cpu_started = thread_cpu_seconds();
        let started = Instant::now();
        let mut client = Side::default();
        while started.elapsed() < duration {
            let sent = Instant::now();
            frame(encoder.encode(format, sample), &mut request);
            stream.write_all(&request).unwrap_or_else(|error| fail("client write", error));
            if !read_frame(&mut stream, &mut payload).unwrap_or_else(|error| fail("client read", error)) {
                eprintln!("tcp: the echo server closed the connection");
                exit(1);
            }
            match format::decode::<M>(format, &payload) {
                Some(decoded) if decoded.matches(sample) => client.messages += 1,
                _ => client.failures += 1,
            }
            round_trips.push(sent.elapsed().as_nanos().min(u128::from(u32::MAX)) as u32);
            client.bytes += request.len() as u64;
        }
        client.elapsed = started.elapsed();
        client.cpu_seconds = thread_cpu_seconds() - cpu_started;
        let _ = stream.shutdown(Shutdown::Write);

        let server = server.join().unwrap_or_else(|_| {
            eprintln!("tcp: the echo server thread panicked");
            exit(1);
        });
        round_trips.sort_unstable();
        (client, server, round_trips)
    })
}

struct Runner {
    duration: Duration,
    formats: Vec<Format>,
    modes: Vec<Mode>,
    stream_rows: Vec<String>,
    echo_rows: Vec<String>,
    failures: u64,
}

impl Visitor for Runner {
    fn visit<M: Pair>(&mut self, sample: &Sample<M>) {
        for mode in self.modes.clone() {
            for format in self.formats.clone() {
                let frame_bytes = HEADER_LEN + Encoder::default().encode(format, sample).len();
                if mode == Mode::Echo {
                    let (client, server, round_trips) = echo(sample, format, self.duration);
                    self.failures += client.failures + server.failures;
                    self.echo_rows.push(format!(
                        "| {} | {} | {} | {} B | {} | {} | {} | {} | {} |",
                        sample.name,
                        sample.data,
                        format.name(),
                        thousands(frame_bytes as f64),
                        thousands(client.messages as f64 / client.elapsed.as_secs_f64()),
                        nanos(percentile(&round_trips, 0.50)),
                        nanos(percentile(&round_trips, 0.99)),
                        nanos(client.cpu_nanos_per_message()),
                        nanos(server.cpu_nanos_per_message()),
                    ));
                } else {
                    let (sender, receiver) = stream(sample, format, mode, self.duration);
                    self.failures += receiver.failures;
                    if receiver.messages + receiver.failures != sender.messages {
                        eprintln!(
                            "tcp: {} {} {} sent {} messages but {} arrived",
                            sample.label(),
                            format.name(),
                            mode.name(),
                            sender.messages,
                            receiver.messages + receiver.failures
                        );
                        self.failures += 1;
                    }
                    let seconds = receiver.elapsed.as_secs_f64();
                    self.stream_rows.push(format!(
                        "| {} | {} | {} | {} | {} B | {} | {} | {} | {} | {:.2} | {:.2} |",
                        sample.name,
                        sample.data,
                        mode.name(),
                        format.name(),
                        thousands(frame_bytes as f64),
                        thousands(receiver.messages as f64 / seconds),
                        thousands(receiver.bytes as f64 / seconds / MIB),
                        nanos(sender.cpu_nanos_per_message()),
                        nanos(receiver.cpu_nanos_per_message()),
                        sender.cores(),
                        receiver.cores(),
                    ));
                }
                eprintln!("tcp: done {} · {} · {}", sample.label(), mode.name(), format.name());
            }
        }
    }
}

fn main() {
    if cli::has_flag("--help") || cli::has_flag("-h") {
        eprintln!(
            "usage: tcp [--messages input,transform,batch-10,batch-100,batch-1000,chat,stats]\n           [--modes buffered,unbuffered,echo] [--formats fomoxa,protobuf] [--seconds 3]"
        );
        exit(2);
    }
    let seconds = cli::flag("--seconds")
        .and_then(|value| value.parse::<f64>().ok())
        .unwrap_or(3.0)
        .max(0.1);
    let formats: Vec<Format> = match cli::flag("--formats") {
        Some(list) => list
            .split(',')
            .map(|name| {
                Format::parse(name).unwrap_or_else(|| {
                    eprintln!("tcp: unknown format {name}");
                    exit(2);
                })
            })
            .collect(),
        None => Format::ALL.to_vec(),
    };
    let modes: Vec<Mode> = match cli::flag("--modes") {
        Some(list) => list
            .split(',')
            .map(|name| {
                Mode::ALL
                    .into_iter()
                    .find(|mode| mode.name() == name)
                    .unwrap_or_else(|| {
                        eprintln!("tcp: unknown mode {name}");
                        exit(2);
                    })
            })
            .collect(),
        None => Mode::ALL.to_vec(),
    };
    let messages = cli::flag("--messages");

    let mut runner = Runner {
        duration: Duration::from_secs_f64(seconds),
        formats,
        modes,
        stream_rows: Vec::new(),
        echo_rows: Vec::new(),
        failures: 0,
    };
    samples::visit_all(&mut runner, messages.as_deref());

    println!("tcp: loopback, one connection, 4-byte length prefix, TCP_NODELAY, {seconds:.1} s per cell");
    if !runner.stream_rows.is_empty() {
        println!();
        println!("One-way stream: the sender encodes and writes as fast as it can, the receiver reads and decodes every frame.");
        println!();
        println!("| Message | Data | Writes | Format | Frame | Messages/s | Wire MiB/s | Sender CPU/msg | Receiver CPU/msg | Sender cores | Receiver cores |");
        println!("|---|---|---|---|---|---|---|---|---|---|---|");
        for row in &runner.stream_rows {
            println!("{row}");
        }
    }
    if !runner.echo_rows.is_empty() {
        println!();
        println!("Echo: the client sends one frame and waits; the server decodes it, encodes it again and sends it back.");
        println!();
        println!("| Message | Data | Format | Frame | Requests/s | p50 round trip | p99 round trip | Client CPU/req | Server CPU/req |");
        println!("|---|---|---|---|---|---|---|---|---|");
        for row in &runner.echo_rows {
            println!("{row}");
        }
    }
    if runner.failures > 0 {
        eprintln!("tcp: {} messages failed to decode or went missing", runner.failures);
        exit(1);
    }
}
