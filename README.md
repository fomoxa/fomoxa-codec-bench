# Fomoxa Codec Bench

A pure format benchmark: Fomoxa against Protobuf (`prost`) on the same messages. It measures:

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

## Quick Start

```sh
git clone https://github.com/fomoxa/fomoxa-codec-bench
cd fomoxa-codec-bench
scripts/run.sh
```

The script builds the project, runs both benchmarks, prints the markdown tables below and writes them to `${TMPDIR:-/tmp}/fomoxa-codec-bench/results.md`. Other ways to run it:

```sh
scripts/run.sh --messages=input,batch-100 --modes=buffered,echo --seconds=5
target/release/codec --messages stats --millis 1000 --repeats 9
target/release/tcp --messages batch-1000 --formats fomoxa --modes echo
```

You need only Rust (`cargo`). Both dependencies come from crates.io:

- `fomoxa-attributes` for the annotations;
- `prost` with derive macros, so `protoc` is not required.

The Fomoxa codec in `src/generated/` is committed together with its runtime, so a build needs no Fomoxa checkout. You need [`fomoxac`](https://github.com/fomoxa/fomoxac) only when you change `src/models/messages.rs`:

```sh
cargo install --git https://github.com/fomoxa/fomoxac
fomoxac generate
```

## The Messages

Each message is declared twice, with the same fields in the same order:

- once as a Fomoxa model in [src/models/messages.rs](src/models/messages.rs);
- once as a `prost` struct in [src/protobuf.rs](src/protobuf.rs).

The Protobuf side uses the types a careful author would pick: `uint32`/`uint64` varints for ids and counters, `fixed32` for the colour, and `float` for floats.

| Message | Fields | Typical data | Sparse data |
|---|---|---|---|
| `input` | `u32` sequence, 2 × `f32` move, `bool` jump, 2 × `f32` look | a moving player, sequence around 1.5 M | a player standing still: every field 0 except sequence 42 |
| `transform` | `u32` id, `u8` kind, `u32` colour, 5 × `f32` | random position within ±500, rotation and colour | - |
| `batch-10`, `batch-100`, `batch-1000` | `u32` sequence + `Array<transform>` | as above, ids 1..n | kind, colour, height and pitch are 0 |
| `chat` | `u32` sender, `u16` channel, `u64` timestamp, 64-character ASCII string | - | - |
| `stats` | `u64` id, `u16` level, 5 × `u32` and `u64` counters | small, realistic values (level 42, 950 health, 17 kills) | - |

All values come from a fixed-seed generator, so every run encodes the same bytes. Before timing, every sample is checked to round-trip in both formats.

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

## What This Benchmark Does Not Cover

- **One implementation per format:** Rust only, `prost` only. The numbers say nothing about protobuf in C++, Go or C#, about other Fomoxa runtimes, or about FlatBuffers, Cap'n Proto or bincode.
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
| Date | 2026-09-17 |

CPU figures are logical CPUs (one hyperthread) as the WSL2 guest sees them, not physical cores. WSL2 presents a synthetic topology, so it is impossible to know or pin whether a thread ran on a P-core or an E-core, and threads were not pinned.

## Layout

```
src/models/messages.rs   the benchmark messages, annotated for fomoxac
src/generated/           fomoxac output, including its runtime
src/protobuf.rs          the same messages as prost structs
src/pair.rs              Fomoxa model ↔ prost struct, per message
src/samples.rs           deterministic typical and sparse samples
src/format.rs            Format, Encoder (reused buffers), decode
src/measure.rs           median timing, percentiles, thread CPU time
src/bin/codec.rs         size, encode and decode table
src/bin/tcp.rs           stream (buffered, unbuffered) and echo over loopback TCP
scripts/run.sh           runs both and writes results.md
```
