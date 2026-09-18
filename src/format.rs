use capnp::message::{Builder, ReaderOptions, SegmentArray};
use capnp::serialize;
use capnp::traits::Owned;
use prost::Message;

use crate::capnproto::ReusedSegment;
use crate::generated::Writer;
use crate::pair::Pair;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Fomoxa,
    Protobuf,
    Capnp,
    CapnpCanonical,
}

impl Format {
    pub const ALL: [Format; 4] = [Format::Fomoxa, Format::Protobuf, Format::Capnp, Format::CapnpCanonical];

    pub fn name(self) -> &'static str {
        match self {
            Format::Fomoxa => "fomoxa",
            Format::Protobuf => "protobuf",
            Format::Capnp => "capnp",
            Format::CapnpCanonical => "capnp-canonical",
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
    capnp: Vec<u8>,
    capnp_segment: ReusedSegment,
    capnp_canonical_segment: ReusedSegment,
}

impl Encoder {
    pub fn encode<M: Pair>(&mut self, format: Format, sample: &Sample<M>) -> &[u8] {
        match format {
            Format::Fomoxa => self.encode_fomoxa(&sample.fomoxa),
            Format::Protobuf => self.encode_protobuf(&sample.proto),
            Format::Capnp => self.encode_capnp(&sample.fomoxa),
            Format::CapnpCanonical => self.encode_capnp_canonical(&sample.fomoxa),
        }
    }

    pub fn encode_decoded<M: Pair>(&mut self, decoded: &Decoded<M>) -> &[u8] {
        match decoded {
            Decoded::Fomoxa(value) => self.encode_fomoxa(value),
            Decoded::Protobuf(value) => self.encode_protobuf(value),
            Decoded::Capnp(value) => self.encode_capnp(value),
            Decoded::CapnpCanonical(value) => self.encode_capnp_canonical(value),
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

    fn encode_capnp<M: Pair>(&mut self, value: &M) -> &[u8] {
        let mut message = Builder::new(&mut self.capnp_segment);
        value.encode_capnp(&mut message);
        self.capnp.clear();
        serialize::write_message(&mut self.capnp, &message).expect("a Vec grows to fit any message");
        &self.capnp
    }

    fn encode_capnp_canonical<M: Pair>(&mut self, value: &M) -> &[u8] {
        let mut draft = Builder::new(&mut self.capnp_segment);
        value.encode_capnp(&mut draft);
        let root = draft
            .get_root_as_reader::<<M::Capnp as Owned>::Reader<'_>>()
            .expect("a message that was just built reads back");
        let mut canonical = Builder::new(&mut self.capnp_canonical_segment);
        canonical
            .set_root_canonical::<M::Capnp>(root)
            .expect("a message that was just built canonicalizes");
        self.capnp.clear();
        self.capnp.extend_from_slice(canonical.get_segments_for_output()[0]);
        &self.capnp
    }
}

pub enum Decoded<M: Pair> {
    Fomoxa(M),
    Protobuf(M::Proto),
    Capnp(M),
    CapnpCanonical(M),
}

impl<M: Pair> Decoded<M> {
    pub fn matches(&self, sample: &Sample<M>) -> bool {
        match self {
            Decoded::Fomoxa(value) | Decoded::Capnp(value) | Decoded::CapnpCanonical(value) => *value == sample.fomoxa,
            Decoded::Protobuf(value) => *value == sample.proto,
        }
    }
}

pub fn decode<M: Pair>(format: Format, bytes: &[u8]) -> Option<Decoded<M>> {
    match format {
        Format::Fomoxa => M::decode_fomoxa(bytes).ok().map(Decoded::Fomoxa),
        Format::Protobuf => M::Proto::decode(bytes).ok().map(Decoded::Protobuf),
        Format::Capnp => decode_capnp::<M>(bytes).ok().map(Decoded::Capnp),
        Format::CapnpCanonical => decode_capnp_canonical::<M>(bytes).ok().map(Decoded::CapnpCanonical),
    }
}

fn decode_capnp<M: Pair>(mut bytes: &[u8]) -> capnp::Result<M> {
    let message = serialize::read_message_from_flat_slice_no_alloc(&mut bytes, ReaderOptions::new())?;
    M::decode_capnp(&message)
}

fn decode_capnp_canonical<M: Pair>(bytes: &[u8]) -> capnp::Result<M> {
    let segments = [bytes];
    let message = capnp::message::Reader::new(SegmentArray::new(&segments), ReaderOptions::new());
    if !bytes.len().is_multiple_of(8) || !message.is_canonical()? {
        return Err(capnp::Error::failed("the message is not canonical".to_owned()));
    }
    M::decode_capnp(&message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capnproto::SEGMENT_WORDS;
    use crate::models::messages::{Input, Transform, TransformBatch};
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

    struct EncodedBy {
        format: Format,
        shared: Encoder,
        encoded: Vec<(String, Vec<u8>)>,
    }

    impl Visitor for EncodedBy {
        fn visit<M: Pair>(&mut self, sample: &Sample<M>) {
            let fresh = Encoder::default().encode(self.format, sample).to_vec();
            let reused = self.shared.encode(self.format, sample).to_vec();
            assert_eq!(reused, fresh, "reused {} encoder differs on {}", self.format.name(), sample.label());
            self.encoded.push((sample.label(), fresh));
        }
    }

    fn encoded_by(format: Format) -> Vec<(String, Vec<u8>)> {
        let mut encoded_by = EncodedBy {
            format,
            shared: Encoder::default(),
            encoded: Vec::new(),
        };
        samples::visit_all(&mut encoded_by, None);
        encoded_by.encoded
    }

    fn sizes(format: Format) -> Vec<(String, usize)> {
        encoded_by(format)
            .into_iter()
            .map(|(label, bytes)| (label, bytes.len()))
            .collect()
    }

    fn labelled(expected: [(&str, usize); 11]) -> Vec<(String, usize)> {
        expected
            .into_iter()
            .map(|(label, size)| (label.to_owned(), size))
            .collect()
    }

    #[test]
    fn every_sample_round_trips_in_every_format() {
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
        assert_eq!(Format::parse("capnp"), Some(Format::Capnp));
        assert_eq!(Format::parse("capnp-canonical"), Some(Format::CapnpCanonical));
        assert_eq!(Format::parse("json"), None);
    }

    #[test]
    fn reused_capnp_encoders_match_fresh_ones_after_larger_messages() {
        assert_eq!(encoded_by(Format::Capnp).len(), 11);
        assert_eq!(encoded_by(Format::CapnpCanonical).len(), 11);
    }

    #[test]
    fn every_capnp_sample_is_a_single_segment_within_the_reused_one() {
        for (label, bytes) in encoded_by(Format::Capnp) {
            let extra_segments = u32::from_le_bytes(bytes[0..4].try_into().unwrap());
            let segment_words = u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize;
            assert_eq!(extra_segments, 0, "{label}");
            assert!(segment_words <= SEGMENT_WORDS, "{label}");
            assert_eq!(bytes.len(), 8 + segment_words * 8, "{label}");
        }
    }

    #[test]
    fn capnp_sizes_follow_the_word_layout() {
        let expected = labelled([
            ("input (typical)", 40),
            ("input (sparse)", 40),
            ("transform (typical)", 48),
            ("batch-10 (typical)", 360),
            ("batch-10 (sparse)", 360),
            ("batch-100 (typical)", 3_240),
            ("batch-100 (sparse)", 3_240),
            ("batch-1000 (typical)", 32_040),
            ("batch-1000 (sparse)", 32_040),
            ("chat (typical)", 112),
            ("stats (typical)", 56),
        ]);
        assert_eq!(sizes(Format::Capnp), expected);
    }

    #[test]
    fn canonical_capnp_drops_the_segment_table_and_trailing_zero_words() {
        let expected = labelled([
            ("input (typical)", 32),
            ("input (sparse)", 16),
            ("transform (typical)", 40),
            ("batch-10 (typical)", 352),
            ("batch-10 (sparse)", 352),
            ("batch-100 (typical)", 3_232),
            ("batch-100 (sparse)", 3_232),
            ("batch-1000 (typical)", 32_032),
            ("batch-1000 (sparse)", 32_032),
            ("chat (typical)", 104),
            ("stats (typical)", 48),
        ]);
        assert_eq!(sizes(Format::CapnpCanonical), expected);
    }

    #[test]
    fn canonical_capnp_matches_the_library_canonicalization() {
        for ((label, plain), (_, canonical)) in encoded_by(Format::Capnp).into_iter().zip(encoded_by(Format::CapnpCanonical)) {
            let message = serialize::read_message_from_flat_slice(&mut &plain[..], ReaderOptions::new()).unwrap();
            let reference = message.canonicalize().unwrap();
            assert_eq!(capnp::Word::words_to_bytes(&reference), &canonical[..], "{label}");
        }
    }

    #[test]
    fn a_non_canonical_segment_is_rejected_in_canonical_mode() {
        let sample = Sample::new("input", samples::SPARSE, Input { sequence: 42, ..Input::default() });
        let mut encoder = Encoder::default();
        let plain_segment = encoder.encode(Format::Capnp, &sample)[8..].to_vec();
        let canonical = encoder.encode(Format::CapnpCanonical, &sample).to_vec();
        assert_ne!(plain_segment, canonical);
        assert!(decode::<Input>(Format::CapnpCanonical, &canonical).is_some());
        assert!(decode::<Input>(Format::CapnpCanonical, &plain_segment).is_none());

        let mut trailing_word = canonical.clone();
        trailing_word.extend_from_slice(&[0; 8]);
        assert!(decode::<Input>(Format::CapnpCanonical, &trailing_word).is_none());
    }

    #[test]
    fn a_truncated_capnp_message_is_rejected() {
        let sample = Sample::new("transform", samples::TYPICAL, Transform::default());
        let mut encoder = Encoder::default();
        for format in [Format::Capnp, Format::CapnpCanonical] {
            let bytes = encoder.encode(format, &sample).to_vec();
            for length in [0, 4, bytes.len() - 1] {
                assert!(decode::<Transform>(format, &bytes[..length]).is_none(), "{} {length} bytes", format.name());
            }
        }
        let batch = Sample::new("batch-10", samples::TYPICAL, TransformBatch {
            sequence: 1,
            transforms: vec![Transform { entity_id: 1, ..Transform::default() }; 10],
        });
        for format in [Format::Capnp, Format::CapnpCanonical] {
            let bytes = encoder.encode(format, &batch).to_vec();
            assert!(decode::<TransformBatch>(format, &bytes[..bytes.len() - 8]).is_none(), "{}", format.name());
        }
    }
}
