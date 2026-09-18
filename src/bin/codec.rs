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
    formats: Vec<Format>,
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
        let measured: Vec<(Format, Measured)> = self
            .formats
            .iter()
            .map(|&format| (format, self.measure(format, sample)))
            .collect();
        let fomoxa = measured
            .iter()
            .find(|(format, _)| *format == Format::Fomoxa)
            .map(|(_, fomoxa)| fomoxa);
        let throughput = |measured: &Measured, nanos: f64| {
            measured.bytes as f64 / nanos * 1e9 / (1024.0 * 1024.0)
        };
        for (format, measured) in &measured {
            let versus = |compare: fn(&Measured, &Measured) -> String| match fomoxa {
                Some(fomoxa) if *format != Format::Fomoxa => compare(fomoxa, measured),
                _ => "-".to_owned(),
            };
            println!(
                "| {} | {} | {} | {} B | {} | {} · {} MiB/s | {} | {} · {} MiB/s | {} |",
                sample.name,
                sample.data,
                format.name(),
                thousands(measured.bytes as f64),
                versus(|fomoxa, other| format!("{:+.0}%", (fomoxa.bytes as f64 / other.bytes as f64 - 1.0) * 100.0)),
                nanos(measured.encode_nanos),
                thousands(throughput(measured, measured.encode_nanos)),
                versus(|fomoxa, other| format!("{:.1}x", other.encode_nanos / fomoxa.encode_nanos)),
                nanos(measured.decode_nanos),
                thousands(throughput(measured, measured.decode_nanos)),
                versus(|fomoxa, other| format!("{:.1}x", other.decode_nanos / fomoxa.decode_nanos)),
            );
        }
    }
}

fn main() {
    if cli::has_flag("--help") || cli::has_flag("-h") {
        eprintln!(
            "usage: codec [--messages input,transform,batch-10,batch-100,batch-1000,chat,stats]\n             [--formats fomoxa,protobuf,capnp,capnp-canonical] [--millis 200] [--repeats 5]"
        );
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
    let formats: Vec<Format> = match cli::flag("--formats") {
        Some(list) => list
            .split(',')
            .map(|name| {
                Format::parse(name).unwrap_or_else(|| {
                    eprintln!("codec: unknown format {name}");
                    exit(2);
                })
            })
            .collect(),
        None => Format::ALL.to_vec(),
    };
    let messages = cli::flag("--messages");

    println!("codec: every cell is the median of {repeats} runs of {millis} ms; the \"Fomoxa vs\" columns compare Fomoxa with the format of that row");
    println!();
    println!("| Message | Data | Format | Size | Fomoxa size vs | Encode | Fomoxa encode speedup | Decode | Fomoxa decode speedup |");
    println!("|---|---|---|---|---|---|---|---|---|");
    samples::visit_all(
        &mut Table {
            duration: Duration::from_millis(millis),
            repeats,
            formats,
        },
        messages.as_deref(),
    );
}
