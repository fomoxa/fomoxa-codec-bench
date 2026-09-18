# Fomoxa Codec Bench

A pure format benchmark: Fomoxa against Protobuf (`prost`) and Cap'n Proto (`capnp`) on the same messages. It measures:

- how many bytes each format produces;
- how fast each one encodes and decodes;
- what that means once the bytes travel over a plain TCP socket.

It contains no game loop, no simulation and no session layer. For a full game workload, see the [Fomoxa Example Game](https://github.com/fomoxa/fomoxa-example-game), which benchmarks Fomoxa only.

## The Short Answer

- **Size depends on the data, not the format name.**
  - With realistic values (floats, random ids, colours), Fomoxa is 17–21% smaller: no field tags, no varint length.
  - With data that is mostly zeros or small integers, Protobuf is smaller, because it omits zero fields and writes small integers as varints. An idle input is 2 B against 21 B, integer stats are 24 B against 38 B, and a sparse batch is 31–35% smaller.
  - Fomoxa always writes fixed-width fields.
- **Speed favours Fomoxa almost everywhere.**
  - Encode is 1.3–4.6x faster and decode 1.4–4.1x faster.
  - The one exception is decoding an almost empty Protobuf message, which has only 2 bytes to read (8.4 ns against 24.0 ns).
- **Over TCP, the bottleneck decides whether the codec matters.**
  - When every message costs one `write()` call, a small message costs about 3.5 µs of syscall and the codec is invisible: 281k against 277k messages/s.
  - When writes are buffered, or messages are large, the codec becomes the bottleneck. Fomoxa then moves 1.5–3.1x more messages per second, and in request/response it cuts round-trip time on a 1000-entity batch from 313 µs to 137 µs.
- **Cap'n Proto depends on the message shape.**
  - On small messages it is the largest (16 bytes of fixed overhead, word padding) and the slowest: 3–5x slower to encode and 2–3x slower to decode than Fomoxa.
  - On arrays of fixed-size structs it is the fastest of the formats here: 2.6x faster encode and 2x faster decode than Fomoxa at `batch-1000`.
  - Its canonical form, the one to hash or sign, is 8 bytes smaller and 2.5–3.3x slower to encode, and it still decodes batches faster than Fomoxa. See [section 4](#4-capn-proto-and-its-canonical-form).

## Quick Start

```sh
git clone https://github.com/fomoxa/fomoxa-codec-bench
cd fomoxa-codec-bench
scripts/run.sh
```

The script builds the project, runs both benchmarks, prints the markdown tables below and writes them to `${TMPDIR:-/tmp}/fomoxa-codec-bench/results.md`. Other ways to run it:

```sh
scripts/run.sh --messages=input,batch-100 --modes=buffered,echo --seconds=5
target/release/codec --messages stats --formats fomoxa,capnp --millis 1000 --repeats 9
target/release/tcp --messages batch-1000 --formats fomoxa --modes echo
```

You need only Rust (`cargo`). All dependencies come from crates.io:

- `fomoxa-attributes` for the annotations;
- `prost` with derive macros, so `protoc` is not required;
- `capnp`, the Cap'n Proto runtime. The code generated from the schema is committed, so the `capnp` compiler is not required either.

The Fomoxa codec in `src/generated/` is committed together with its runtime, so a build needs no Fomoxa checkout. You need [`fomoxac`](https://github.com/fomoxa/fomoxac) only when you change `src/models/messages.rs`:

```sh
cargo install fomoxac
fomoxac generate
```

In the same way, you need the [Cap'n Proto compiler](https://capnproto.org/install.html) and its Rust plugin only when you change `src/messages.capnp`:

```sh
cargo install capnpc
capnp compile -orust:src --src-prefix=src src/messages.capnp
```

## The Messages

Each message is declared three times, with the same fields in the same order:

- once as a Fomoxa model in [src/models/messages.rs](src/models/messages.rs);
- once as a `prost` struct in [src/protobuf.rs](src/protobuf.rs);
- once as a Cap'n Proto struct in [src/messages.capnp](src/messages.capnp).

The Protobuf side uses the types a careful author would pick: `uint32`/`uint64` varints for ids and counters, `fixed32` for the colour, and `float` for floats. Cap'n Proto has no varints, so its schema uses the same widths as Fomoxa: `UInt8`, `UInt16`, `UInt32`, `UInt64`, `Float32`, `Bool`, `Text` and `List(Transform)`.

| Message | Fields | Typical data | Sparse data |
|---|---|---|---|
| `input` | `u32` sequence, 2 × `f32` move, `bool` jump, 2 × `f32` look | a moving player, sequence around 1.5 M | a player standing still: every field 0 except sequence 42 |
| `transform` | `u32` id, `u8` kind, `u32` colour, 5 × `f32` | random position within ±500, rotation and colour | - |
| `batch-10`, `batch-100`, `batch-1000` | `u32` sequence + `Array<transform>` | as above, ids 1..n | kind, colour, height and pitch are 0 |
| `chat` | `u32` sender, `u16` channel, `u64` timestamp, 64-character ASCII string | - | - |
| `stats` | `u64` id, `u16` level, 5 × `u32` and `u64` counters | small, realistic values (level 42, 950 health, 17 kills) | - |

All values come from a fixed-seed generator, so every run encodes the same bytes. Before timing, every sample is checked to round-trip in every format.

## Results

All results come from `scripts/run.sh` on the machine listed under [Test Machine](#test-machine), on 2026-09-17.

### 1. Size and Codec Speed

Each timing is the median of 5 runs of 500 ms.

- **Encoding** writes into a reused buffer in both formats (`Writer::clear` for Fomoxa, `Vec::clear` + `Message::encode` for Protobuf), so no allocation is timed.
- **Decoding** builds the owned struct, including its `Vec`s and `String`s.
- **"Size vs Protobuf"** is Fomoxa's size relative to Protobuf's: negative means Fomoxa is smaller.
- **Speedup** is Protobuf's time divided by Fomoxa's: above 1x means Fomoxa is faster.

| Message | Data | Fomoxa | Protobuf | Size vs Protobuf | Fomoxa encode | Protobuf encode | Encode speedup | Fomoxa decode | Protobuf decode | Decode speedup |
|---|---|---|---|---|---|---|---|---|---|---|
| input | typical | 21 B | 26 B | **−19%** | 7.5 ns | 18.5 ns | 2.5x | 23.1 ns | 38.9 ns | 1.7x |
| input | sparse | 21 B | 2 B | +950% | 7.5 ns | 9.7 ns | 1.3x | 24.0 ns | 8.4 ns | **0.4x** |
| transform | typical | 29 B | 35 B | **−17%** | 9.2 ns | 19.2 ns | 2.1x | 15.5 ns | 60.1 ns | 3.9x |
| batch-10 | typical | 298 B | 358 B | **−17%** | 83 ns | 281 ns | 3.4x | 211 ns | 791 ns | 3.8x |
| batch-10 | sparse | 298 B | 194 B | +54% | 84 ns | 251 ns | 3.0x | 211 ns | 491 ns | 2.3x |
| batch-100 | typical | 2,908 B | 3,586 B | **−19%** | 0.80 µs | 2.70 µs | 3.4x | 1.85 µs | 7.13 µs | 3.8x |
| batch-100 | sparse | 2,908 B | 1,904 B | +53% | 0.81 µs | 1.86 µs | 2.3x | 1.87 µs | 4.09 µs | 2.2x |
| batch-1000 | typical | 29,008 B | 36,625 B | **−21%** | 10.7 µs | 30.8 µs | 2.9x | 19.2 µs | 69.9 µs | 3.6x |
| batch-1000 | sparse | 29,008 B | 19,877 B | +46% | 10.2 µs | 19.5 µs | 1.9x | 19.1 µs | 40.1 µs | 2.1x |
| chat | typical | 82 B | 79 B | +4% | 9.2 ns | 26.8 ns | 2.9x | 51.0 ns | 71.8 ns | 1.4x |
| stats | typical | 38 B | 24 B | +58% | 9.2 ns | 41.9 ns | 4.6x | 18.3 ns | 74.5 ns | 4.1x |

Where the bytes go:

- **Fomoxa** writes every field at its declared width, in order, with no tags. A `transform` is always 4 + 1 + 4 + 5 × 4 = 29 bytes, and a string or array costs a 4-byte length.
- **Protobuf** adds a tag byte to every field that is present, 8 bytes per `transform`; the varint id wins back 2, so a `transform` is 35 bytes. In exchange:
  - it drops fields equal to zero;
  - it shrinks integers with varints (`level = 42` takes 1 byte instead of 2, `kills = 17` takes 1 instead of 4);
  - it uses 1-byte lengths for short strings;
  - each nested message costs a 2-byte length.

In encode and decode time, Fomoxa's fixed layout means the encoder is a sequence of `extend_from_slice` calls and the decoder a sequence of bounds-checked reads. Protobuf pays for tag dispatch and varint branches, per field, in both directions.

In byte throughput on `batch-1000` (typical), Fomoxa encodes at 2.5 GiB/s and decodes at 1.4 GiB/s, while `prost` encodes at 1.1 GiB/s and decodes at 0.49 GiB/s. The raw output of `scripts/run.sh` gives MiB/s for every cell.

### 2. Plain TCP

The TCP test uses:

- one loopback connection;
- a 4-byte little-endian length prefix per frame;
- `TCP_NODELAY`;
- 3 seconds per cell.

"Frame" is the payload plus the 4-byte prefix. CPU is thread CPU time (`utime + stime` from `/proc/thread-self/stat`) divided by messages, and it includes time spent in the kernel.

#### One-Way Stream

The sender encodes and writes as fast as it can, and the receiver reads through a 64 KiB `BufReader` and decodes every frame. The two write strategies:

- **`buffered`:** the sender writes through a 64 KiB `BufWriter`, which is what a server does when it batches.
- **`unbuffered`:** every frame is its own `write()` call.

| Message | Data | Writes | Format | Frame | Messages/s | Wire MiB/s | Sender CPU/msg | Receiver CPU/msg |
|---|---|---|---|---|---|---|---|---|
| input | typical | buffered | fomoxa | 25 B | **31.3 M** | 747 | 29 ns | 32 ns |
| input | typical | buffered | protobuf | 30 B | 21.4 M | 613 | 47 ns | 44 ns |
| input | sparse | buffered | fomoxa | 25 B | 30.5 M | 728 | 29 ns | 33 ns |
| input | sparse | buffered | protobuf | 6 B | **38.6 M** | 221 | 26 ns | 19 ns |
| transform | typical | buffered | fomoxa | 33 B | **25.3 M** | 796 | 40 ns | 29 ns |
| transform | typical | buffered | protobuf | 39 B | 13.6 M | 504 | 61 ns | 73 ns |
| batch-10 | typical | buffered | fomoxa | 302 B | **3.79 M** | 1,092 | 171 ns | 262 ns |
| batch-10 | typical | buffered | protobuf | 362 B | 1.22 M | 420 | 512 ns | 817 ns |
| batch-100 | typical | buffered | fomoxa | 2,912 B | **428 k** | 1,188 | 1.45 µs | 2.33 µs |
| batch-100 | typical | buffered | protobuf | 3,590 B | 139 k | 474 | 4.91 µs | 7.19 µs |
| batch-100 | sparse | buffered | fomoxa | 2,912 B | **432 k** | 1,200 | 1.42 µs | 2.31 µs |
| batch-100 | sparse | buffered | protobuf | 1,908 B | 240 k | 436 | 2.94 µs | 4.15 µs |
| batch-1000 | typical | buffered | fomoxa | 29,012 B | **42.9 k** | 1,188 | 17.1 µs | 23.2 µs |
| batch-1000 | typical | buffered | protobuf | 36,629 B | 13.9 k | 484 | 55.2 µs | 71.9 µs |
| batch-1000 | sparse | buffered | fomoxa | 29,012 B | **42.8 k** | 1,183 | 17.0 µs | 23.3 µs |
| batch-1000 | sparse | buffered | protobuf | 19,881 B | 24.4 k | 463 | 29.5 µs | 40.8 µs |
| chat | typical | buffered | fomoxa | 86 B | **15.3 M** | 1,258 | 44 ns | 65 ns |
| chat | typical | buffered | protobuf | 83 B | 10.4 M | 822 | 67 ns | 96 ns |
| stats | typical | buffered | fomoxa | 42 B | **27.0 M** | 1,080 | 37 ns | 32 ns |
| stats | typical | buffered | protobuf | 28 B | 10.6 M | 284 | 84 ns | 94 ns |
| input | typical | unbuffered | fomoxa | 25 B | 281 k | 7 | 3.56 µs | 2.55 µs |
| input | typical | unbuffered | protobuf | 30 B | 277 k | 8 | 3.62 µs | 2.58 µs |
| transform | typical | unbuffered | fomoxa | 33 B | 273 k | 9 | 3.67 µs | 2.51 µs |
| transform | typical | unbuffered | protobuf | 39 B | 274 k | 10 | 3.65 µs | 2.61 µs |
| chat | typical | unbuffered | fomoxa | 86 B | 279 k | 23 | 3.58 µs | 2.61 µs |
| chat | typical | unbuffered | protobuf | 83 B | 274 k | 22 | 3.64 µs | 2.72 µs |
| stats | typical | unbuffered | fomoxa | 42 B | 284 k | 11 | 3.52 µs | 2.44 µs |
| stats | typical | unbuffered | protobuf | 28 B | 271 k | 7 | 3.69 µs | 2.58 µs |
| batch-10 | typical | unbuffered | fomoxa | 302 B | **264 k** | 76 | 3.78 µs | 2.89 µs |
| batch-10 | typical | unbuffered | protobuf | 362 B | 227 k | 78 | 4.40 µs | 3.96 µs |
| batch-100 | typical | unbuffered | fomoxa | 2,912 B | **192 k** | 532 | 5.22 µs | 4.91 µs |
| batch-100 | typical | unbuffered | protobuf | 3,590 B | 123 k | 422 | 8.12 µs | 8.04 µs |
| batch-1000 | typical | unbuffered | fomoxa | 29,012 B | **42.9 k** | 1,186 | 16.5 µs | 23.2 µs |
| batch-1000 | typical | unbuffered | protobuf | 36,629 B | 13.8 k | 483 | 53.7 µs | 71.9 µs |

How to read it:

- **Buffered: the codec sets the ceiling.**
  - Writes are amortised, and in almost every buffered row one side sits at 0.99–1.00 cores. That side is usually the receiver, which decodes.
  - Fomoxa moves 1.5x (`input`) to 3.1x (`batch-10`, `batch-100`, `batch-1000`) more messages per second.
  - Its wire throughput levels off near 1.2 GiB/s, set by decoding. Protobuf levels off near 0.45 GiB/s.
  - In this mode the sparse idle `input` is the one case where Protobuf wins (38.6 M against 30.5 M messages/s): its 2-byte payload is cheaper to decode than Fomoxa's 21 bytes.
- **Unbuffered, small messages: the syscall sets the ceiling.**
  - Every message costs about 3.5 µs of sender CPU, almost all of it in `write()`.
  - Both formats land at 270–285 k messages/s, and the codec's 10–40 ns is under 2% of the cost.
- **Unbuffered, larger messages: the codec shows through again.**
  - From `batch-10` upward the codec is a visible share of each message, and Fomoxa leads by 16% (`batch-10`), 56% (`batch-100`) and 3.1x (`batch-1000`).
  - At `batch-1000` the unbuffered and buffered rows are identical, because each frame is large enough that decoding dominates either way.

#### Request and Response (Echo)

The client sends one frame and waits for the reply. The server decodes the frame, encodes the decoded value again and sends it back, and the client decodes the reply and checks that it equals what was sent. Both sides use one `write()` per frame; this is a closed loop on one connection.

| Message | Data | Format | Frame | Requests/s | p50 round trip | p99 round trip | Client CPU/req | Server CPU/req |
|---|---|---|---|---|---|---|---|---|
| input | typical | fomoxa | 25 B | 14,830 | 59 µs | 169 µs | 29.4 µs | 28.8 µs |
| input | typical | protobuf | 30 B | 15,042 | 58 µs | 162 µs | 29.7 µs | 28.8 µs |
| input | sparse | fomoxa | 25 B | 14,970 | 59 µs | 166 µs | 28.9 µs | 28.7 µs |
| input | sparse | protobuf | 6 B | 14,932 | 59 µs | 163 µs | 28.8 µs | 29.2 µs |
| transform | typical | fomoxa | 33 B | 13,805 | 61 µs | 190 µs | 31.6 µs | 30.7 µs |
| transform | typical | protobuf | 39 B | 14,275 | 61 µs | 170 µs | 30.1 µs | 30.1 µs |
| chat | typical | fomoxa | 86 B | 14,469 | 60 µs | 186 µs | 30.0 µs | 30.4 µs |
| chat | typical | protobuf | 83 B | 14,782 | 60 µs | 154 µs | 29.8 µs | 29.1 µs |
| stats | typical | fomoxa | 42 B | 14,003 | 60 µs | 221 µs | 30.5 µs | 31.4 µs |
| stats | typical | protobuf | 28 B | 14,892 | 60 µs | 155 µs | 30.0 µs | 29.1 µs |
| batch-10 | typical | fomoxa | 302 B | 14,635 | 59 µs | 167 µs | 30.5 µs | 29.6 µs |
| batch-10 | typical | protobuf | 362 B | 13,262 | 65 µs | 190 µs | 33.7 µs | 32.9 µs |
| batch-100 | typical | fomoxa | 2,912 B | **13,283** | **67 µs** | 167 µs | 34.1 µs | 32.9 µs |
| batch-100 | typical | protobuf | 3,590 B | 9,896 | 91 µs | 206 µs | 46.8 µs | 45.5 µs |
| batch-100 | sparse | fomoxa | 2,912 B | **13,186** | **68 µs** | 165 µs | 34.1 µs | 33.1 µs |
| batch-100 | sparse | protobuf | 1,908 B | 11,563 | 78 µs | 172 µs | 39.8 µs | 38.3 µs |
| batch-1000 | typical | fomoxa | 29,012 B | **6,603** | **137 µs** | 286 µs | 72.7 µs | 68.2 µs |
| batch-1000 | typical | protobuf | 36,629 B | 2,999 | 313 µs | 556 µs | 164 µs | 156 µs |
| batch-1000 | sparse | fomoxa | 29,012 B | **6,612** | **136 µs** | 292 µs | 74.1 µs | 67.6 µs |
| batch-1000 | sparse | protobuf | 19,881 B | 4,461 | 206 µs | 406 µs | 109 µs | 103 µs |

The results fall into two groups:

- **Messages below about 100 bytes:** round trips are about 59–61 µs whichever format is used. The four syscalls and the loopback wakeups cost about 30 µs of CPU on each side, and the codec cost is noise. The p99 differences in these rows (154–221 µs) move from run to run and are scheduler jitter, not format.
- **Messages in the kilobytes:** the codec is a large share of the round trip. Fomoxa gives 26% lower p50 at `batch-100` and 2.3x lower p50 at `batch-1000`, even when the sparse Protobuf frame is a third smaller.

### 3. What the Size Difference Costs in Bandwidth

On a real network, bytes are what you pay for. The table multiplies the measured frame sizes (payload + 4-byte prefix) by an example send rate, before TCP/IP headers:

| Traffic | Fomoxa | Protobuf | Difference |
|---|---|---|---|
| 1000 clients × `input` × 30 Hz, typical | 732 KiB/s | 879 KiB/s | Fomoxa −17% |
| 1000 clients × `input` × 30 Hz, idle | 732 KiB/s | 176 KiB/s | Protobuf −76% |
| 1 server → 100 clients × `batch-100` × 20 Hz, typical | 5.55 MiB/s | 6.85 MiB/s | Fomoxa −19% |
| 1 server → 100 clients × `batch-100` × 20 Hz, sparse | 5.55 MiB/s | 3.64 MiB/s | Protobuf −34% |
| 10,000 `stats` updates/s | 410 KiB/s | 273 KiB/s | Protobuf −33% |

If your data is mostly floats and random ids, Fomoxa is both smaller and faster. If it is mostly zeros, flags and small counters, Protobuf is smaller on the wire, and Fomoxa's speed advantage only pays off where the CPU, not the network, is the limit.

### 4. Cap'n Proto and Its Canonical Form

Cap'n Proto was added on 2026-09-18, and this section comes from one `scripts/run.sh` run on that day, on the same machine. That run was slower than the one in sections 1–3 for Fomoxa and Protobuf too (Fomoxa decodes `batch-1000` in 25.2 µs here against 19.2 µs above), so compare formats within this section only.

Two Cap'n Proto formats are measured:

- **`capnp`** is the standard unpacked serialization.
  - It is written with `capnp::serialize::write_message` into a reused `Vec`.
  - The builder uses a reused 64 KiB first segment ([src/capnproto.rs](src/capnproto.rs)), so like the other two formats no allocation is timed, and every sample fits in a single segment.
  - Decoding reads the bytes in place with `read_message_from_flat_slice_no_alloc` and then builds the same owned struct as Fomoxa, including its `Vec`s and `String`s.
- **`capnp-canonical`** is the [canonical form](https://capnproto.org/encoding.html#canonicalization): one segment, objects in pre-order, and trailing zero words cut from every struct, so a value has exactly one encoding. It is what you hash or sign.
  - Encoding builds the message as `capnp` does, then copies it with `set_root_canonical` into a second reused segment. The frame is that segment alone: a canonical message has one segment, so no segment table is sent.
  - Decoding reads the frame as one segment, checks it with `is_canonical()` and rejects anything that is not canonical, then builds the owned struct. A receiver that relies on the bytes being canonical has to make that check.

`codec` prints one row per format. "Fomoxa size vs" is Fomoxa's size relative to the format of that row, and the speedups are that format's time divided by Fomoxa's.

| Message | Data | Format | Size | Fomoxa size vs | Encode | Fomoxa encode speedup | Decode | Fomoxa decode speedup |
|---|---|---|---|---|---|---|---|---|
| input | typical | fomoxa | 21 B | - | 9.1 ns · 2,189 MiB/s | - | 25.1 ns · 797 MiB/s | - |
| input | typical | protobuf | 26 B | -19% | 28.0 ns · 885 MiB/s | 3.1x | 34.5 ns · 718 MiB/s | 1.4x |
| input | typical | capnp | 40 B | -48% | 43.4 ns · 879 MiB/s | 4.7x | 77.2 ns · 494 MiB/s | 3.1x |
| input | typical | capnp-canonical | 32 B | -34% | 141 ns · 217 MiB/s | 15.4x | 111 ns · 274 MiB/s | 4.4x |
| input | sparse | fomoxa | 21 B | - | 9.1 ns · 2,204 MiB/s | - | 25.0 ns · 802 MiB/s | - |
| input | sparse | protobuf | 2 B | +950% | 12.7 ns · 150 MiB/s | 1.4x | 14.8 ns · 129 MiB/s | 0.6x |
| input | sparse | capnp | 40 B | -48% | 43.1 ns · 884 MiB/s | 4.7x | 76.9 ns · 496 MiB/s | 3.1x |
| input | sparse | capnp-canonical | 16 B | +31% | 109 ns · 139 MiB/s | 12.0x | 110 ns · 139 MiB/s | 4.4x |
| transform | typical | fomoxa | 29 B | - | 11.1 ns · 2,489 MiB/s | - | 30.2 ns · 916 MiB/s | - |
| transform | typical | protobuf | 35 B | -17% | 58.3 ns · 572 MiB/s | 5.3x | 55.6 ns · 601 MiB/s | 1.8x |
| transform | typical | capnp | 48 B | -40% | 42.9 ns · 1,068 MiB/s | 3.9x | 87.4 ns · 524 MiB/s | 2.9x |
| transform | typical | capnp-canonical | 40 B | -28% | 110 ns · 346 MiB/s | 9.9x | 115 ns · 332 MiB/s | 3.8x |
| batch-10 | typical | fomoxa | 298 B | - | 73.1 ns · 3,888 MiB/s | - | 281 ns · 1,010 MiB/s | - |
| batch-10 | typical | protobuf | 358 B | -17% | 665 ns · 514 MiB/s | 9.1x | 793 ns · 430 MiB/s | 2.8x |
| batch-10 | typical | capnp | 360 B | -17% | 84.5 ns · 4,063 MiB/s | 1.2x | 249 ns · 1,381 MiB/s | 0.9x |
| batch-10 | typical | capnp-canonical | 352 B | -15% | 218 ns · 1,542 MiB/s | 3.0x | 340 ns · 988 MiB/s | 1.2x |
| batch-10 | sparse | fomoxa | 298 B | - | 73.4 ns · 3,871 MiB/s | - | 282 ns · 1,007 MiB/s | - |
| batch-10 | sparse | protobuf | 194 B | +54% | 335 ns · 552 MiB/s | 4.6x | 538 ns · 344 MiB/s | 1.9x |
| batch-10 | sparse | capnp | 360 B | -17% | 85.3 ns · 4,023 MiB/s | 1.2x | 251 ns · 1,367 MiB/s | 0.9x |
| batch-10 | sparse | capnp-canonical | 352 B | -15% | 222 ns · 1,515 MiB/s | 3.0x | 343 ns · 979 MiB/s | 1.2x |
| batch-100 | typical | fomoxa | 2,908 B | - | 684 ns · 4,056 MiB/s | - | 2.56 µs · 1,082 MiB/s | - |
| batch-100 | typical | protobuf | 3,586 B | -19% | 6.40 µs · 534 MiB/s | 9.4x | 6.58 µs · 519 MiB/s | 2.6x |
| batch-100 | typical | capnp | 3,240 B | -10% | 289 ns · 10,706 MiB/s | 0.4x | 1.39 µs · 2,222 MiB/s | 0.5x |
| batch-100 | typical | capnp-canonical | 3,232 B | -10% | 847 ns · 3,638 MiB/s | 1.2x | 1.58 µs · 1,955 MiB/s | 0.6x |
| batch-100 | sparse | fomoxa | 2,908 B | - | 687 ns · 4,035 MiB/s | - | 2.57 µs · 1,081 MiB/s | - |
| batch-100 | sparse | protobuf | 1,904 B | +53% | 3.23 µs · 562 MiB/s | 4.7x | 3.96 µs · 459 MiB/s | 1.5x |
| batch-100 | sparse | capnp | 3,240 B | -10% | 284 ns · 10,866 MiB/s | 0.4x | 1.40 µs · 2,214 MiB/s | 0.5x |
| batch-100 | sparse | capnp-canonical | 3,232 B | -10% | 862 ns · 3,576 MiB/s | 1.3x | 1.59 µs · 1,934 MiB/s | 0.6x |
| batch-1000 | typical | fomoxa | 29,008 B | - | 8.67 µs · 3,191 MiB/s | - | 25.18 µs · 1,099 MiB/s | - |
| batch-1000 | typical | protobuf | 36,625 B | -21% | 72.35 µs · 483 MiB/s | 8.3x | 64.90 µs · 538 MiB/s | 2.6x |
| batch-1000 | typical | capnp | 32,040 B | -9% | 3.35 µs · 9,125 MiB/s | 0.4x | 12.58 µs · 2,428 MiB/s | 0.5x |
| batch-1000 | typical | capnp-canonical | 32,032 B | -9% | 8.75 µs · 3,491 MiB/s | 1.0x | 13.75 µs · 2,222 MiB/s | 0.5x |
| batch-1000 | sparse | fomoxa | 29,008 B | - | 9.45 µs · 2,927 MiB/s | - | 25.08 µs · 1,103 MiB/s | - |
| batch-1000 | sparse | protobuf | 19,877 B | +46% | 31.20 µs · 608 MiB/s | 3.3x | 36.41 µs · 521 MiB/s | 1.5x |
| batch-1000 | sparse | capnp | 32,040 B | -9% | 3.31 µs · 9,223 MiB/s | 0.4x | 12.92 µs · 2,366 MiB/s | 0.5x |
| batch-1000 | sparse | capnp-canonical | 32,032 B | -9% | 8.75 µs · 3,490 MiB/s | 0.9x | 13.84 µs · 2,207 MiB/s | 0.6x |
| chat | typical | fomoxa | 82 B | - | 20.2 ns · 3,872 MiB/s | - | 48.9 ns · 1,599 MiB/s | - |
| chat | typical | protobuf | 79 B | +4% | 33.8 ns · 2,231 MiB/s | 1.7x | 73.7 ns · 1,022 MiB/s | 1.5x |
| chat | typical | capnp | 112 B | -27% | 58.9 ns · 1,815 MiB/s | 2.9x | 118 ns · 905 MiB/s | 2.4x |
| chat | typical | capnp-canonical | 104 B | -21% | 165 ns · 601 MiB/s | 8.2x | 197 ns · 503 MiB/s | 4.0x |
| stats | typical | fomoxa | 38 B | - | 10.8 ns · 3,369 MiB/s | - | 29.0 ns · 1,251 MiB/s | - |
| stats | typical | protobuf | 24 B | +58% | 74.8 ns · 306 MiB/s | 7.0x | 76.1 ns · 301 MiB/s | 2.6x |
| stats | typical | capnp | 56 B | -32% | 49.2 ns · 1,085 MiB/s | 4.6x | 66.9 ns · 798 MiB/s | 2.3x |
| stats | typical | capnp-canonical | 48 B | -21% | 121 ns · 379 MiB/s | 11.2x | 111 ns · 413 MiB/s | 3.8x |

Where Cap'n Proto's bytes go:

- a `capnp` message starts with an 8-byte segment table and an 8-byte root pointer;
- fields sit at their natural width inside 64-bit words, and every struct is padded to whole words, so a `transform` is 4 words (32 B), an `input` 3 words and `stats` 5 words;
- a list costs a pointer and a tag word, and text costs a pointer, a NUL byte and padding to 8 bytes;
- zero fields still take their space, so sparse and typical data are the same size;
- the canonical form drops the segment table (8 bytes less on every message) and cuts trailing zero words. That only matters for the idle `input`, whose last two words are zero: it shrinks from 40 B to 16 B, smaller than Fomoxa's 21 B. The sparse `transform`s keep a non-zero `yaw` in their last word, so they stay 32 B each.

One-way stream, buffered writes, typical data (selected rows):

| Message | Data | Writes | Format | Frame | Messages/s | Wire MiB/s | Sender CPU/msg | Receiver CPU/msg |
|---|---|---|---|---|---|---|---|---|
| input | typical | buffered | fomoxa | 25 B | 28,459,459 | 679 | 29.8 ns | 35.0 ns |
| input | typical | buffered | capnp | 44 B | 11,197,369 | 470 | 61.0 ns | 88.9 ns |
| input | typical | buffered | capnp-canonical | 36 B | 7,346,841 | 252 | 136 ns | 127 ns |
| transform | typical | buffered | fomoxa | 33 B | 25,440,060 | 801 | 32.5 ns | 39.2 ns |
| transform | typical | buffered | capnp | 52 B | 10,045,104 | 498 | 63.1 ns | 99.1 ns |
| transform | typical | buffered | capnp-canonical | 44 B | 6,988,530 | 293 | 143 ns | 130 ns |
| batch-10 | typical | buffered | fomoxa | 302 B | 3,056,112 | 880 | 153 ns | 325 ns |
| batch-10 | typical | buffered | capnp | 364 B | 3,268,252 | 1,135 | 167 ns | 305 ns |
| batch-10 | typical | buffered | capnp-canonical | 356 B | 2,455,141 | 834 | 337 ns | 406 ns |
| batch-100 | typical | buffered | fomoxa | 2,912 B | 336,658 | 935 | 1.41 µs | 2.96 µs |
| batch-100 | typical | buffered | capnp | 3,244 B | 525,029 | 1,624 | 1.01 µs | 1.90 µs |
| batch-100 | typical | buffered | capnp-canonical | 3,236 B | 475,609 | 1,468 | 1.63 µs | 2.09 µs |
| batch-1000 | typical | buffered | fomoxa | 29,012 B | 34,670 | 959 | 15.82 µs | 28.66 µs |
| batch-1000 | typical | buffered | capnp | 32,044 B | 56,698 | 1,733 | 10.80 µs | 17.55 µs |
| batch-1000 | typical | buffered | capnp-canonical | 32,036 B | 53,153 | 1,624 | 17.28 µs | 18.72 µs |
| chat | typical | buffered | fomoxa | 86 B | 14,407,226 | 1,182 | 43.9 ns | 69.2 ns |
| chat | typical | buffered | capnp | 116 B | 6,947,440 | 769 | 97.6 ns | 143 ns |
| chat | typical | buffered | capnp-canonical | 108 B | 4,476,243 | 461 | 204 ns | 223 ns |
| stats | typical | buffered | fomoxa | 42 B | 22,308,904 | 894 | 33.4 ns | 44.6 ns |
| stats | typical | buffered | capnp | 60 B | 12,802,018 | 733 | 62.6 ns | 77.9 ns |
| stats | typical | buffered | capnp-canonical | 52 B | 6,756,119 | 335 | 148 ns | 128 ns |

Echo, typical data (selected rows):

| Message | Data | Format | Frame | Requests/s | p50 round trip | p99 round trip | Client CPU/req | Server CPU/req |
|---|---|---|---|---|---|---|---|---|
| batch-100 | typical | fomoxa | 2,912 B | 12,405 | 69.53 µs | 220 µs | 35.47 µs | 35.74 µs |
| batch-100 | typical | protobuf | 3,590 B | 9,593 | 92.73 µs | 220 µs | 47.60 µs | 46.56 µs |
| batch-100 | typical | capnp | 3,244 B | 12,842 | 66.89 µs | 209 µs | 34.52 µs | 33.74 µs |
| batch-100 | typical | capnp-canonical | 3,236 B | 12,412 | 71.03 µs | 174 µs | 35.99 µs | 34.64 µs |
| batch-1000 | typical | fomoxa | 29,012 B | 6,303 | 146 µs | 278 µs | 77.21 µs | 70.86 µs |
| batch-1000 | typical | protobuf | 36,629 B | 2,829 | 337 µs | 486 µs | 173 µs | 165 µs |
| batch-1000 | typical | capnp | 32,044 B | 7,825 | 115 µs | 242 µs | 61.34 µs | 55.38 µs |
| batch-1000 | typical | capnp-canonical | 32,036 B | 7,024 | 129 µs | 262 µs | 67.86 µs | 64.06 µs |

How to read it:

- **Small messages: Cap'n Proto is the largest and the slowest.**
  - The fixed 16 bytes and the word padding make `input` 40 B (Fomoxa 21 B, Protobuf 26 B), `stats` 56 B and `chat` 112 B.
  - Every message pays for the builder, the segment table and pointer validation: 43–59 ns to encode and 67–118 ns to decode, against 9–20 ns and 25–49 ns for Fomoxa.
  - In buffered streams it moves 6.9–12.8 M small messages/s, 40–60% of Fomoxa. Unbuffered and in echo the syscall hides it, as it hides the other formats.
- **Arrays of fixed-size structs: Cap'n Proto is the fastest.**
  - A list of structs is written and read in place, one word-aligned element after another. At `batch-100` and `batch-1000` it encodes 2.4–2.9x faster than Fomoxa (3.35 µs against 8.67 µs for 1000 entities) and decodes 1.8–2x faster (12.6 µs against 25.2 µs).
  - The crossover is around `batch-10`, where Cap'n Proto is 16% slower to encode and 11% faster to decode.
  - Buffered, it moves 1.6x more `batch-100` and `batch-1000` messages per second than Fomoxa, and it cuts the `batch-1000` echo p50 to 115 µs, against 146 µs for Fomoxa and 337 µs for Protobuf.
  - Its batches are 10–11% larger than Fomoxa's and 10–13% smaller than typical Protobuf, but 1.6–1.7x the size of sparse Protobuf.
- **The canonical form costs a second copy on encode and a validation pass on decode.**
  - Encoding is 2.5–3.3x slower than plain `capnp`: 109–165 ns for small messages, and 8.75 µs for `batch-1000`, about the same as Fomoxa.
  - Decoding costs 28–79 ns more on small messages and 9–14% more on batches, so canonical Cap'n Proto still decodes `batch-100` and `batch-1000` 1.6–1.8x faster than Fomoxa.
  - Buffered, small canonical messages move at 4.5–7.3 M messages/s, 53–70% of plain `capnp`, and the sender now uses 0.9–1.0 cores. Batches lose less: `batch-1000` moves 53.2 k messages/s against 56.7 k for plain `capnp` and 34.7 k for Fomoxa, with a 129 µs echo p50.
  - Size alone is not a reason to choose it: it saves 8 bytes per message, except on mostly-zero structs.
- **Decoding builds an owned struct.** That keeps the comparison equal, but it is not how Cap'n Proto is usually read: reading fields straight from the buffer, without the copy into `Vec<Transform>`, is cheaper and is not measured here.

## What This Benchmark Does Not Cover

- **One implementation per format:** Rust only, `prost` and `capnp` only. The numbers say nothing about Protobuf or Cap'n Proto in C++, Go or C#, about other Fomoxa runtimes, or about FlatBuffers or bincode.
- **Cap'n Proto packed encoding:** only the unpacked serialization is measured. Packing would shrink the zero bytes and padding, at extra CPU on both sides.
- **No general-purpose compression:** there is no zstd or deflate on top of either format. Compression would narrow the size gap on repetitive data and add CPU to both.
- **Loopback, not a network:** traffic never crosses a NIC, so there is no packet loss, no MTU, no congestion control and no WAN latency. TCP/IP header overhead is not included in the bandwidth table.
- **No session layer:** there is no handshake, schema negotiation, message ids or heartbeats; framing is a bare length prefix. The `fomoxa-net` runtime adds those, and its cost is measured in the example game, not here.
- **No schema evolution cost:** both formats can add fields at the end, but the benchmark encodes one schema version only.
- **Synthetic data:** the samples are shaped like game traffic but are generated. Run the tool on your own message shapes before drawing conclusions for your protocol.
- **Timing on a shared machine:** each timing is a median, but WSL2 and a laptop CPU with P-cores and E-cores add noise, especially to p99 and to sub-10 ns cells.

## Test Machine

| | |
|---|---|
| CPU | 13th Gen Intel Core i5-13500HX: 14 physical cores (6 P + 8 E), 20 logical CPUs |
| RAM | 7 GiB assigned to WSL2 |
| OS | WSL2, kernel 6.6.87.2-microsoft-standard-WSL2, on Windows |
| Network | loopback inside the guest, no NIC |
| Build | rustc 1.98.0, `cargo build --release` |
| Date | 2026-09-17 (sections 1–3), 2026-09-18 (section 4) |

CPU figures are logical CPUs (one hyperthread) as the WSL2 guest sees them, not physical cores. WSL2 presents a synthetic topology, so it is impossible to know or pin whether a thread ran on a P-core or an E-core, and threads were not pinned.

## Layout

```
src/models/messages.rs   the benchmark messages, annotated for fomoxac
src/generated/           fomoxac output, including its runtime
src/protobuf.rs          the same messages as prost structs
src/messages.capnp       the same messages as a Cap'n Proto schema
src/messages_capnp.rs    capnpc-rust output for that schema
src/capnproto.rs         a reused segment for the Cap'n Proto builders
src/pair.rs              Fomoxa model ↔ prost struct and Cap'n Proto builder/reader, per message
src/samples.rs           deterministic typical and sparse samples
src/format.rs            Format, Encoder (reused buffers), decode
src/measure.rs           median timing, percentiles, thread CPU time
src/bin/codec.rs         size, encode and decode table
src/bin/tcp.rs           stream (buffered, unbuffered) and echo over loopback TCP
scripts/run.sh           runs both and writes results.md
```
