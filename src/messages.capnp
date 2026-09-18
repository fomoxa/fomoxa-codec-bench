@0xe9df1bf5b446527a;

struct Input {
  sequence @0 :UInt32;
  moveX @1 :Float32;
  moveZ @2 :Float32;
  jump @3 :Bool;
  lookYaw @4 :Float32;
  lookPitch @5 :Float32;
}

struct Transform {
  entityId @0 :UInt32;
  kind @1 :UInt8;
  color @2 :UInt32;
  positionX @3 :Float32;
  positionY @4 :Float32;
  positionZ @5 :Float32;
  yaw @6 :Float32;
  pitch @7 :Float32;
}

struct TransformBatch {
  sequence @0 :UInt32;
  transforms @1 :List(Transform);
}

struct Chat {
  senderId @0 :UInt32;
  channel @1 :UInt16;
  sentAtMillis @2 :UInt64;
  text @3 :Text;
}

struct Stats {
  playerId @0 :UInt64;
  level @1 :UInt16;
  health @2 :UInt32;
  mana @3 :UInt32;
  gold @4 :UInt32;
  experience @5 :UInt64;
  kills @6 :UInt32;
  deaths @7 :UInt32;
}
