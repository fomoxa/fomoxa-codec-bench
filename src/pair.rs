use capnp::message::{Allocator, Builder, Reader as CapnpReader, ReaderSegments};

use crate::generated::{
    ChatBenchCodec, DecodeError, InputBenchCodec, Reader, StatsBenchCodec, TransformBatchBenchCodec,
    TransformBenchCodec, Writer,
};
use crate::messages_capnp::{chat, input, stats, transform, transform_batch};
use crate::models::messages::{Chat, Input, Stats, Transform, TransformBatch};
use crate::protobuf;

pub trait Pair: Clone + Default + PartialEq + std::fmt::Debug + Send + Sync + 'static {
    type Proto: prost::Message + Default + Clone + PartialEq + std::fmt::Debug + Send + Sync + 'static;
    type Capnp: capnp::traits::Owned;

    fn to_proto(&self) -> Self::Proto;
    fn encode_fomoxa(&self, writer: &mut Writer);
    fn decode_fomoxa(bytes: &[u8]) -> Result<Self, DecodeError>;
    fn encode_capnp<A: Allocator>(&self, message: &mut Builder<A>);
    fn decode_capnp<S: ReaderSegments>(message: &CapnpReader<S>) -> capnp::Result<Self>;
}

fn write_transform(transform: &Transform, mut builder: transform::Builder<'_>) {
    builder.set_entity_id(transform.entity_id);
    builder.set_kind(transform.kind);
    builder.set_color(transform.color);
    builder.set_position_x(transform.position_x);
    builder.set_position_y(transform.position_y);
    builder.set_position_z(transform.position_z);
    builder.set_yaw(transform.yaw);
    builder.set_pitch(transform.pitch);
}

fn read_transform(reader: transform::Reader<'_>) -> Transform {
    Transform {
        entity_id: reader.get_entity_id(),
        kind: reader.get_kind(),
        color: reader.get_color(),
        position_x: reader.get_position_x(),
        position_y: reader.get_position_y(),
        position_z: reader.get_position_z(),
        yaw: reader.get_yaw(),
        pitch: reader.get_pitch(),
    }
}

macro_rules! fomoxa_codec {
    ($codec:ident) => {
        fn encode_fomoxa(&self, writer: &mut Writer) {
            $codec::encode(writer, self);
        }

        fn decode_fomoxa(bytes: &[u8]) -> Result<Self, DecodeError> {
            let mut value = Self::default();
            $codec::decode(&mut Reader::new(bytes), &mut value)?;
            Ok(value)
        }
    };
}

impl Pair for Input {
    type Proto = protobuf::Input;
    type Capnp = input::Owned;
    fomoxa_codec!(InputBenchCodec);

    fn to_proto(&self) -> protobuf::Input {
        protobuf::Input {
            sequence: self.sequence,
            move_x: self.move_x,
            move_z: self.move_z,
            jump: self.jump,
            look_yaw: self.look_yaw,
            look_pitch: self.look_pitch,
        }
    }

    fn encode_capnp<A: Allocator>(&self, message: &mut Builder<A>) {
        let mut root = message.init_root::<input::Builder>();
        root.set_sequence(self.sequence);
        root.set_move_x(self.move_x);
        root.set_move_z(self.move_z);
        root.set_jump(self.jump);
        root.set_look_yaw(self.look_yaw);
        root.set_look_pitch(self.look_pitch);
    }

    fn decode_capnp<S: ReaderSegments>(message: &CapnpReader<S>) -> capnp::Result<Input> {
        let root = message.get_root::<input::Reader>()?;
        Ok(Input {
            sequence: root.get_sequence(),
            move_x: root.get_move_x(),
            move_z: root.get_move_z(),
            jump: root.get_jump(),
            look_yaw: root.get_look_yaw(),
            look_pitch: root.get_look_pitch(),
        })
    }
}

impl Pair for Transform {
    type Proto = protobuf::Transform;
    type Capnp = transform::Owned;
    fomoxa_codec!(TransformBenchCodec);

    fn to_proto(&self) -> protobuf::Transform {
        protobuf::Transform {
            entity_id: self.entity_id,
            kind: u32::from(self.kind),
            color: self.color,
            position_x: self.position_x,
            position_y: self.position_y,
            position_z: self.position_z,
            yaw: self.yaw,
            pitch: self.pitch,
        }
    }

    fn encode_capnp<A: Allocator>(&self, message: &mut Builder<A>) {
        write_transform(self, message.init_root());
    }

    fn decode_capnp<S: ReaderSegments>(message: &CapnpReader<S>) -> capnp::Result<Transform> {
        Ok(read_transform(message.get_root()?))
    }
}

impl Pair for TransformBatch {
    type Proto = protobuf::TransformBatch;
    type Capnp = transform_batch::Owned;
    fomoxa_codec!(TransformBatchBenchCodec);

    fn to_proto(&self) -> protobuf::TransformBatch {
        protobuf::TransformBatch {
            sequence: self.sequence,
            transforms: self.transforms.iter().map(Pair::to_proto).collect(),
        }
    }

    fn encode_capnp<A: Allocator>(&self, message: &mut Builder<A>) {
        let mut root = message.init_root::<transform_batch::Builder>();
        root.set_sequence(self.sequence);
        let mut transforms = root.init_transforms(self.transforms.len() as u32);
        for (index, transform) in self.transforms.iter().enumerate() {
            write_transform(transform, transforms.reborrow().get(index as u32));
        }
    }

    fn decode_capnp<S: ReaderSegments>(message: &CapnpReader<S>) -> capnp::Result<TransformBatch> {
        let root = message.get_root::<transform_batch::Reader>()?;
        Ok(TransformBatch {
            sequence: root.get_sequence(),
            transforms: root.get_transforms()?.iter().map(read_transform).collect(),
        })
    }
}

impl Pair for Chat {
    type Proto = protobuf::Chat;
    type Capnp = chat::Owned;
    fomoxa_codec!(ChatBenchCodec);

    fn to_proto(&self) -> protobuf::Chat {
        protobuf::Chat {
            sender_id: self.sender_id,
            channel: u32::from(self.channel),
            sent_at_millis: self.sent_at_millis,
            text: self.text.clone(),
        }
    }

    fn encode_capnp<A: Allocator>(&self, message: &mut Builder<A>) {
        let mut root = message.init_root::<chat::Builder>();
        root.set_sender_id(self.sender_id);
        root.set_channel(self.channel);
        root.set_sent_at_millis(self.sent_at_millis);
        root.set_text(self.text.as_str());
    }

    fn decode_capnp<S: ReaderSegments>(message: &CapnpReader<S>) -> capnp::Result<Chat> {
        let root = message.get_root::<chat::Reader>()?;
        Ok(Chat {
            sender_id: root.get_sender_id(),
            channel: root.get_channel(),
            sent_at_millis: root.get_sent_at_millis(),
            text: root.get_text()?.to_string()?,
        })
    }
}

impl Pair for Stats {
    type Proto = protobuf::Stats;
    type Capnp = stats::Owned;
    fomoxa_codec!(StatsBenchCodec);

    fn to_proto(&self) -> protobuf::Stats {
        protobuf::Stats {
            player_id: self.player_id,
            level: u32::from(self.level),
            health: self.health,
            mana: self.mana,
            gold: self.gold,
            experience: self.experience,
            kills: self.kills,
            deaths: self.deaths,
        }
    }

    fn encode_capnp<A: Allocator>(&self, message: &mut Builder<A>) {
        let mut root = message.init_root::<stats::Builder>();
        root.set_player_id(self.player_id);
        root.set_level(self.level);
        root.set_health(self.health);
        root.set_mana(self.mana);
        root.set_gold(self.gold);
        root.set_experience(self.experience);
        root.set_kills(self.kills);
        root.set_deaths(self.deaths);
    }

    fn decode_capnp<S: ReaderSegments>(message: &CapnpReader<S>) -> capnp::Result<Stats> {
        let root = message.get_root::<stats::Reader>()?;
        Ok(Stats {
            player_id: root.get_player_id(),
            level: root.get_level(),
            health: root.get_health(),
            mana: root.get_mana(),
            gold: root.get_gold(),
            experience: root.get_experience(),
            kills: root.get_kills(),
            deaths: root.get_deaths(),
        })
    }
}
