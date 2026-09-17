use fomoxa_attributes::*;

#[network]
#[codec(bench)]
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Input {
    #[network(u32)]
    #[codec(bench)]
    pub sequence: u32,

    #[network(f32)]
    #[codec(bench)]
    pub move_x: f32,

    #[network(f32)]
    #[codec(bench)]
    pub move_z: f32,

    #[network(bool)]
    #[codec(bench)]
    pub jump: bool,

    #[network(f32)]
    #[codec(bench)]
    pub look_yaw: f32,

    #[network(f32)]
    #[codec(bench)]
    pub look_pitch: f32,
}

#[network]
#[codec(bench)]
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Transform {
    #[network(u32)]
    #[codec(bench)]
    pub entity_id: u32,

    #[network(u8)]
    #[codec(bench)]
    pub kind: u8,

    #[network(u32)]
    #[codec(bench)]
    pub color: u32,

    #[network(f32)]
    #[codec(bench)]
    pub position_x: f32,

    #[network(f32)]
    #[codec(bench)]
    pub position_y: f32,

    #[network(f32)]
    #[codec(bench)]
    pub position_z: f32,

    #[network(f32)]
    #[codec(bench)]
    pub yaw: f32,

    #[network(f32)]
    #[codec(bench)]
    pub pitch: f32,
}

#[network]
#[codec(bench)]
#[derive(Debug, Default, Clone, PartialEq)]
pub struct TransformBatch {
    #[network(u32)]
    #[codec(bench)]
    pub sequence: u32,

    #[network(Array<Transform>)]
    #[codec(bench)]
    pub transforms: Vec<Transform>,
}

#[network]
#[codec(bench)]
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Chat {
    #[network(u32)]
    #[codec(bench)]
    pub sender_id: u32,

    #[network(u16)]
    #[codec(bench)]
    pub channel: u16,

    #[network(u64)]
    #[codec(bench)]
    pub sent_at_millis: u64,

    #[network(string)]
    #[codec(bench)]
    pub text: String,
}

#[network]
#[codec(bench)]
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Stats {
    #[network(u64)]
    #[codec(bench)]
    pub player_id: u64,

    #[network(u16)]
    #[codec(bench)]
    pub level: u16,

    #[network(u32)]
    #[codec(bench)]
    pub health: u32,

    #[network(u32)]
    #[codec(bench)]
    pub mana: u32,

    #[network(u32)]
    #[codec(bench)]
    pub gold: u32,

    #[network(u64)]
    #[codec(bench)]
    pub experience: u64,

    #[network(u32)]
    #[codec(bench)]
    pub kills: u32,

    #[network(u32)]
    #[codec(bench)]
    pub deaths: u32,
}
