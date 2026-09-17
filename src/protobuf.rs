use prost::Message;

#[derive(Clone, PartialEq, Message)]
pub struct Input {
    #[prost(uint32, tag = "1")]
    pub sequence: u32,
    #[prost(float, tag = "2")]
    pub move_x: f32,
    #[prost(float, tag = "3")]
    pub move_z: f32,
    #[prost(bool, tag = "4")]
    pub jump: bool,
    #[prost(float, tag = "5")]
    pub look_yaw: f32,
    #[prost(float, tag = "6")]
    pub look_pitch: f32,
}

#[derive(Clone, PartialEq, Message)]
pub struct Transform {
    #[prost(uint32, tag = "1")]
    pub entity_id: u32,
    #[prost(uint32, tag = "2")]
    pub kind: u32,
    #[prost(fixed32, tag = "3")]
    pub color: u32,
    #[prost(float, tag = "4")]
    pub position_x: f32,
    #[prost(float, tag = "5")]
    pub position_y: f32,
    #[prost(float, tag = "6")]
    pub position_z: f32,
    #[prost(float, tag = "7")]
    pub yaw: f32,
    #[prost(float, tag = "8")]
    pub pitch: f32,
}

#[derive(Clone, PartialEq, Message)]
pub struct TransformBatch {
    #[prost(uint32, tag = "1")]
    pub sequence: u32,
    #[prost(message, repeated, tag = "2")]
    pub transforms: Vec<Transform>,
}

#[derive(Clone, PartialEq, Message)]
pub struct Chat {
    #[prost(uint32, tag = "1")]
    pub sender_id: u32,
    #[prost(uint32, tag = "2")]
    pub channel: u32,
    #[prost(uint64, tag = "3")]
    pub sent_at_millis: u64,
    #[prost(string, tag = "4")]
    pub text: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct Stats {
    #[prost(uint64, tag = "1")]
    pub player_id: u64,
    #[prost(uint32, tag = "2")]
    pub level: u32,
    #[prost(uint32, tag = "3")]
    pub health: u32,
    #[prost(uint32, tag = "4")]
    pub mana: u32,
    #[prost(uint32, tag = "5")]
    pub gold: u32,
    #[prost(uint64, tag = "6")]
    pub experience: u64,
    #[prost(uint32, tag = "7")]
    pub kills: u32,
    #[prost(uint32, tag = "8")]
    pub deaths: u32,
}
