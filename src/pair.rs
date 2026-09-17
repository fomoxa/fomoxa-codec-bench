use crate::generated::{
    ChatBenchCodec, DecodeError, InputBenchCodec, Reader, StatsBenchCodec, TransformBatchBenchCodec,
    TransformBenchCodec, Writer,
};
use crate::models::messages::{Chat, Input, Stats, Transform, TransformBatch};
use crate::protobuf;

pub trait Pair: Clone + Default + PartialEq + std::fmt::Debug + Send + Sync + 'static {
    type Proto: prost::Message + Default + Clone + PartialEq + std::fmt::Debug + Send + Sync + 'static;

    fn to_proto(&self) -> Self::Proto;
    fn encode_fomoxa(&self, writer: &mut Writer);
    fn decode_fomoxa(bytes: &[u8]) -> Result<Self, DecodeError>;
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
}

impl Pair for Transform {
    type Proto = protobuf::Transform;
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
}

impl Pair for TransformBatch {
    type Proto = protobuf::TransformBatch;
    fomoxa_codec!(TransformBatchBenchCodec);

    fn to_proto(&self) -> protobuf::TransformBatch {
        protobuf::TransformBatch {
            sequence: self.sequence,
            transforms: self.transforms.iter().map(Pair::to_proto).collect(),
        }
    }
}

impl Pair for Chat {
    type Proto = protobuf::Chat;
    fomoxa_codec!(ChatBenchCodec);

    fn to_proto(&self) -> protobuf::Chat {
        protobuf::Chat {
            sender_id: self.sender_id,
            channel: u32::from(self.channel),
            sent_at_millis: self.sent_at_millis,
            text: self.text.clone(),
        }
    }
}

impl Pair for Stats {
    type Proto = protobuf::Stats;
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
}
