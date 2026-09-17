use std::f32::consts::PI;

use crate::format::Sample;
use crate::models::messages::{Chat, Input, Stats, Transform, TransformBatch};
use crate::pair::Pair;

pub const TYPICAL: &str = "typical";
pub const SPARSE: &str = "sparse";

const CHAT_TEXT: &str = "gg, regroup at the north gate in 30 seconds and bring the healer";

pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Rng {
        Rng(seed.max(1))
    }

    pub fn next_u32(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 32) as u32
    }

    pub fn unit(&mut self) -> f32 {
        self.next_u32() as f32 / u32::MAX as f32
    }

    pub fn range(&mut self, low: f32, high: f32) -> f32 {
        low + (high - low) * self.unit()
    }
}

pub trait Visitor {
    fn visit<M: Pair>(&mut self, sample: &Sample<M>);
}

fn typical_input(rng: &mut Rng) -> Input {
    let heading = rng.range(-PI, PI);
    Input {
        sequence: 1_000_000 + rng.next_u32() % 1_000_000,
        move_x: heading.cos(),
        move_z: heading.sin(),
        jump: rng.next_u32() % 4 == 0,
        look_yaw: heading,
        look_pitch: rng.range(-0.5, 0.5),
    }
}

fn typical_transform(rng: &mut Rng, entity_id: u32) -> Transform {
    Transform {
        entity_id,
        kind: (rng.next_u32() % 8) as u8,
        color: rng.next_u32() | 0xFF,
        position_x: rng.range(-500.0, 500.0),
        position_y: rng.range(0.0, 20.0),
        position_z: rng.range(-500.0, 500.0),
        yaw: rng.range(-PI, PI),
        pitch: rng.range(-1.5, 1.5),
    }
}

fn sparse_transform(rng: &mut Rng, entity_id: u32) -> Transform {
    Transform {
        entity_id,
        kind: 0,
        color: 0,
        position_x: rng.range(-500.0, 500.0),
        position_y: 0.0,
        position_z: rng.range(-500.0, 500.0),
        yaw: rng.range(-PI, PI),
        pitch: 0.0,
    }
}

fn batch(rng: &mut Rng, count: u32, transform: fn(&mut Rng, u32) -> Transform) -> TransformBatch {
    TransformBatch {
        sequence: 1_000_000 + rng.next_u32() % 1_000_000,
        transforms: (1..=count).map(|entity_id| transform(rng, entity_id)).collect(),
    }
}

pub fn visit_all(visitor: &mut impl Visitor, filter: Option<&str>) {
    let mut rng = Rng::new(0x5EED_F0E0_A11C_E5ED);
    let wanted = |name: &str| filter.map_or(true, |filter| filter.split(',').any(|wanted| wanted == name));

    let input = typical_input(&mut rng);
    if wanted("input") {
        visitor.visit(&Sample::new("input", TYPICAL, input));
        visitor.visit(&Sample::new(
            "input",
            SPARSE,
            Input {
                sequence: 42,
                ..Input::default()
            },
        ));
    }

    let transform = typical_transform(&mut rng, 4_242);
    if wanted("transform") {
        visitor.visit(&Sample::new("transform", TYPICAL, transform));
    }

    for (name, count) in [("batch-10", 10), ("batch-100", 100), ("batch-1000", 1000)] {
        let typical = batch(&mut rng, count, typical_transform);
        let sparse = batch(&mut rng, count, sparse_transform);
        if wanted(name) {
            visitor.visit(&Sample::new(name, TYPICAL, typical));
            visitor.visit(&Sample::new(name, SPARSE, sparse));
        }
    }

    if wanted("chat") {
        visitor.visit(&Sample::new(
            "chat",
            TYPICAL,
            Chat {
                sender_id: 73_411,
                channel: 3,
                sent_at_millis: 1_789_632_000_123,
                text: CHAT_TEXT.to_owned(),
            },
        ));
    }

    if wanted("stats") {
        visitor.visit(&Sample::new(
            "stats",
            TYPICAL,
            Stats {
                player_id: 123_456_789,
                level: 42,
                health: 950,
                mana: 300,
                gold: 12_345,
                experience: 1_234_567,
                kills: 17,
                deaths: 5,
            },
        ));
    }
}
