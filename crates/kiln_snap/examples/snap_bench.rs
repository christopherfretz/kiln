//! A hand-run throughput check of the encoder (spec/snap.md
//! "Performance"): `cargo run --release -p kiln_snap --example snap_bench`.
//! Prints encode and decode MB/s for the fixed-size fast path, the same
//! records through the general path, short strings and an Fx map.

use std::collections::HashMap;
use std::hash::BuildHasherDefault;
use std::hint::black_box;
use std::time::Instant;

use kiln_snap::{Reader, Schema, Snap, SnapError, Writer, from_bytes, to_bytes};

/// A 16-byte fixed-size record: the fast path.
#[derive(Clone, Copy, PartialEq, Debug, Snap)]
struct Rec {
    a: u32,
    b: u32,
    c: u64,
}

/// The same record without a fixed size: the general path, one `get` per
/// item with its own bounds checks.
#[derive(Clone, Copy, PartialEq, Debug)]
struct Slow(Rec);

impl Snap for Slow {
    const SCHEMA_HASH: u64 = Rec::SCHEMA_HASH;
    fn put(&self, w: &mut Writer) {
        self.0.put(w);
    }
    fn get(r: &mut Reader<'_>) -> Result<Self, SnapError> {
        Rec::get(r).map(Slow)
    }
    fn schema(s: &mut Schema) {
        Rec::schema(s);
    }
}

/// An `FxHash` copy (the workspace's own lives in `kiln_source`).
#[derive(Default, Clone, Copy)]
struct Fx(u64);

impl std::hash::Hasher for Fx {
    fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.write_u64(u64::from(b));
        }
    }
    fn write_u32(&mut self, i: u32) {
        self.write_u64(u64::from(i));
    }
    fn write_u64(&mut self, i: u64) {
        self.0 = (self.0.rotate_left(5) ^ i).wrapping_mul(0x517c_c1b7_2722_0a95);
    }
    fn finish(&self) -> u64 {
        self.0
    }
}

/// Best of `ROUNDS` encodes and decodes of `v`, in MB/s of encoded bytes.
fn bench<T: Snap + PartialEq>(name: &str, v: &T) {
    const ROUNDS: usize = 7;
    let bytes = to_bytes(v);
    let (mut enc, mut dec) = (f64::MAX, f64::MAX);
    for _ in 0..ROUNDS {
        let t = Instant::now();
        black_box(to_bytes(black_box(v)));
        enc = enc.min(t.elapsed().as_secs_f64());
        let t = Instant::now();
        let back: T = from_bytes(black_box(&bytes)).unwrap();
        dec = dec.min(t.elapsed().as_secs_f64());
        assert!(back == *v);
    }
    let mb = f64::from(u32::try_from(bytes.len()).unwrap_or(u32::MAX)) / 1e6;
    println!(
        "{name:<34} {:>9} bytes  encode {:>8.0} MB/s  decode {:>8.0} MB/s",
        bytes.len(),
        mb / enc,
        mb / dec
    );
}

fn main() {
    let recs: Vec<Rec> = (0..1_000_000u32)
        .map(|i| Rec { a: i, b: i.wrapping_mul(7), c: u64::from(i) << 20 })
        .collect();
    bench("1M 16-byte records, fast path", &recs);
    let slow: Vec<Slow> = recs.iter().copied().map(Slow).collect();
    bench("1M 16-byte records, general path", &slow);
    let strings: Vec<String> = (0..100_000u32).map(|i| format!("ident_{i}")).collect();
    bench("100k short strings", &strings);
    let map: HashMap<u32, u32, BuildHasherDefault<Fx>> =
        (0..100_000u32).map(|i| (i.wrapping_mul(2_654_435_761), i)).collect();
    bench("Fx map, 100k u32 -> u32", &map);
}
