use std::hint::black_box;
use std::process::exit;
use std::time::Duration;

use fomoxa_codec_bench::cli;
use fomoxa_codec_bench::format::{self, Encoder, Format, Sample};
use fomoxa_codec_bench::measure::{nanos, nanos_per_call, thousands};
use fomoxa_codec_bench::pair::Pair;
use fomoxa_codec_bench::samples::{self, Visitor};

struct Measured {
    bytes: usize,
    encode_nanos: f64,
    decode_nanos: f64,
}

struct Table {
    duration: Duration,
    repeats: usize,
}

impl Table {
    fn measure<M: Pair>(&self, format: Format, sample: &Sample<M>) -> Measured {
        let mut encoder = Encoder::default();
        let bytes = encoder.encode(format, sample).to_vec();
        match format::decode::<M>(format, &bytes) {
            Some(decoded) if decoded.matches(sample) => {}
            _ => {
                eprintln!("codec: {} does not round-trip {}", format.name(), sample.label());
                exit(1);
            }
        }

        let encode_nanos = nanos_per_call(self.duration, self.repeats, || {
            black_box(encoder.encode(format, black_box(sample)).len());
        });
        let decode_nanos = nanos_per_call(self.duration, self.repeats, || {
            black_box(format::decode::<M>(format, black_box(&bytes)).is_some());
        });
        Measured {
            bytes: bytes.len(),
            encode_nanos,
            decode_nanos,
        }
    }
}

impl Visitor for Table {
    fn visit<M: Pair>(&mut self, sample: &Sample<M>) {
        let fomoxa = self.measure(Format::Fomoxa, sample);
        let protobuf = self.measure(Format::Protobuf, sample);
        let saved = (1.0 - fomoxa.bytes as f64 / protobuf.bytes as f64) * 100.0;
        let throughput = |measured: &Measured, nanos: f64| {
            measured.bytes as f64 / nanos * 1e9 / (1024.0 * 1024.0)
        };
        println!(
            "| {} | {} | {} B | {} B | {:+.0}% | {} · {} MiB/s | {} · {} MiB/s | {:.1}x | {} · {} MiB/s | {} · {} MiB/s | {:.1}x |",
            sample.name,
            sample.data,
            thousands(fomoxa.bytes as f64),
            thousands(protobuf.bytes as f64),
            -saved,
            nanos(fomoxa.encode_nanos),
            thousands(throughput(&fomoxa, fomoxa.encode_nanos)),
            nanos(protobuf.encode_nanos),
            thousands(throughput(&protobuf, protobuf.encode_nanos)),
            protobuf.encode_nanos / fomoxa.encode_nanos,
            nanos(fomoxa.decode_nanos),
            thousands(throughput(&fomoxa, fomoxa.decode_nanos)),
            nanos(protobuf.decode_nanos),
            thousands(throughput(&protobuf, protobuf.decode_nanos)),
            protobuf.decode_nanos / fomoxa.decode_nanos,
        );
    }
}

fn main() {
    if cli::has_flag("--help") || cli::has_flag("-h") {
        eprintln!("usage: codec [--messages input,transform,batch-10,batch-100,batch-1000,chat,stats] [--millis 200] [--repeats 5]");
        exit(2);
    }
    let millis = cli::flag("--millis")
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(200)
        .max(1);
    let repeats = cli::flag("--repeats")
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(5)
        .max(1);
    let messages = cli::flag("--messages");

    println!("codec: every cell is the median of {repeats} runs of {millis} ms; size change is Fomoxa relative to Protobuf");
    println!();
    println!("| Message | Data | Fomoxa size | Protobuf size | Size vs Protobuf | Fomoxa encode | Protobuf encode | Encode speedup | Fomoxa decode | Protobuf decode | Decode speedup |");
    println!("|---|---|---|---|---|---|---|---|---|---|---|");
    samples::visit_all(
        &mut Table {
            duration: Duration::from_millis(millis),
            repeats,
        },
        messages.as_deref(),
    );
}
