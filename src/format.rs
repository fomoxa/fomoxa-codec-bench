use prost::Message;

use crate::generated::Writer;
use crate::pair::Pair;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Fomoxa,
    Protobuf,
}

impl Format {
    pub const ALL: [Format; 2] = [Format::Fomoxa, Format::Protobuf];

    pub fn name(self) -> &'static str {
        match self {
            Format::Fomoxa => "fomoxa",
            Format::Protobuf => "protobuf",
        }
    }

    pub fn parse(name: &str) -> Option<Format> {
        Format::ALL.into_iter().find(|format| format.name() == name)
    }
}

pub struct Sample<M: Pair> {
    pub name: &'static str,
    pub data: &'static str,
    pub fomoxa: M,
    pub proto: M::Proto,
}

impl<M: Pair> Sample<M> {
    pub fn new(name: &'static str, data: &'static str, value: M) -> Sample<M> {
        Sample {
            name,
            data,
            proto: value.to_proto(),
            fomoxa: value,
        }
    }

    pub fn label(&self) -> String {
        format!("{} ({})", self.name, self.data)
    }
}

#[derive(Default)]
pub struct Encoder {
    writer: Writer,
    proto: Vec<u8>,
}

impl Encoder {
    pub fn encode<M: Pair>(&mut self, format: Format, sample: &Sample<M>) -> &[u8] {
        match format {
            Format::Fomoxa => self.encode_fomoxa(&sample.fomoxa),
            Format::Protobuf => self.encode_protobuf(&sample.proto),
        }
    }

    pub fn encode_decoded<M: Pair>(&mut self, decoded: &Decoded<M>) -> &[u8] {
        match decoded {
            Decoded::Fomoxa(value) => self.encode_fomoxa(value),
            Decoded::Protobuf(value) => self.encode_protobuf(value),
        }
    }

    fn encode_fomoxa<M: Pair>(&mut self, value: &M) -> &[u8] {
        self.writer.clear();
        value.encode_fomoxa(&mut self.writer);
        self.writer.as_slice()
    }

    fn encode_protobuf<P: Message>(&mut self, value: &P) -> &[u8] {
        self.proto.clear();
        value
            .encode(&mut self.proto)
            .expect("a Vec grows to fit any message");
        &self.proto
    }
}

pub enum Decoded<M: Pair> {
    Fomoxa(M),
    Protobuf(M::Proto),
}

impl<M: Pair> Decoded<M> {
    pub fn matches(&self, sample: &Sample<M>) -> bool {
        match self {
            Decoded::Fomoxa(value) => *value == sample.fomoxa,
            Decoded::Protobuf(value) => *value == sample.proto,
        }
    }
}

pub fn decode<M: Pair>(format: Format, bytes: &[u8]) -> Option<Decoded<M>> {
    match format {
        Format::Fomoxa => M::decode_fomoxa(bytes).ok().map(Decoded::Fomoxa),
        Format::Protobuf => M::Proto::decode(bytes).ok().map(Decoded::Protobuf),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::samples::{self, Visitor};

    struct RoundTrip {
        visited: usize,
    }

    impl Visitor for RoundTrip {
        fn visit<M: Pair>(&mut self, sample: &Sample<M>) {
            let mut encoder = Encoder::default();
            for format in Format::ALL {
                let bytes = encoder.encode(format, sample).to_vec();
                let decoded = decode::<M>(format, &bytes).expect("every sample decodes");
                assert!(decoded.matches(sample), "{} {}", format.name(), sample.label());
                assert_eq!(encoder.encode_decoded(&decoded), &bytes[..]);
            }
            self.visited += 1;
        }
    }

    #[test]
    fn every_sample_round_trips_in_both_formats() {
        let mut round_trip = RoundTrip { visited: 0 };
        samples::visit_all(&mut round_trip, None);
        assert_eq!(round_trip.visited, 11);
    }

    #[test]
    fn a_filter_selects_exact_message_names() {
        let mut round_trip = RoundTrip { visited: 0 };
        samples::visit_all(&mut round_trip, Some("batch-10,stats"));
        assert_eq!(round_trip.visited, 3);
    }

    #[test]
    fn format_names_parse() {
        assert_eq!(Format::parse("fomoxa"), Some(Format::Fomoxa));
        assert_eq!(Format::parse("protobuf"), Some(Format::Protobuf));
        assert_eq!(Format::parse("json"), None);
    }
}
