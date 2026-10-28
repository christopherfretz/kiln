//! Differential test: seeded random class definitions are laid out and
//! dumped by `kiln_abi` and by `clang++-19 -Xclang -fdump-record-layouts`
//! for each supported target, and every dump block must match byte for
//! byte (spec/abi.md §7). Skipped when `clang++-19` is not installed.
//!
//! `KILN_ABI_DIFF_SEED` overrides the seed; `KILN_ABI_DIFF_RECORDS` the
//! number of records per target; `KILN_ABI_DIFF_SHOW` prints every
//! generated source and kiln dump.

mod common;

use common::{
    CLANG, EnumDef, FieldDef, Item, MARKER, Model, RecDef, Ty, TypedefDef, blocks, clang_available,
    run_clang,
};
use kiln_abi::Layouter;
use kiln_abi::target::{Target, target};
use kiln_abi::write_record_layout;
use kiln_ast::decl::TagKind;
use kiln_ast::types::BuiltinType as B;

/// Records per generated translation unit.
const PER_TU: usize = 40;
/// Concurrent clang processes.
const JOBS: usize = 8;

/// Known divergences: `(record name, reason)`. Empty: none are known.
const XFAIL: &[(&str, &str)] = &[];

/// A small deterministic PRNG (`SplitMix64`).
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
    fn chance(&mut self, percent: u64) -> bool {
        self.below(100) < percent
    }
    fn pick<T: Copy>(&mut self, xs: &[T]) -> T {
        xs[usize::try_from(self.below(xs.len() as u64)).expect("index")]
    }
}

/// What the generator knows about each declared item.
#[derive(Clone, Copy, Default)]
#[allow(clippy::struct_excessive_bools, reason = "independent generator facts")]
struct Facts {
    /// A record (else an enum or typedef).
    record: bool,
    enumeration: bool,
    /// Has a flexible array member (never used as a member).
    fam: bool,
    /// The implicit default constructor is usable.
    dc: bool,
    /// The default constructor is non-trivial.
    nontrivial: bool,
    /// Over-aligned typedef (never an array element).
    over_aligned: bool,
    /// Declares no members but unnamed bit-fields (an empty class).
    empty: bool,
    /// An upper bound on the alignment, in bytes.
    align_ub: u64,
}

struct Gen<'t> {
    rng: Rng,
    target: &'t Target,
    model: Model,
    facts: Vec<Facts>,
}

const SCALARS: &[B] = &[
    B::Bool,
    B::Char,
    B::SChar,
    B::UChar,
    B::WChar,
    B::Char8,
    B::Char16,
    B::Char32,
    B::Short,
    B::UShort,
    B::Int,
    B::UInt,
    B::Long,
    B::ULong,
    B::LongLong,
    B::ULongLong,
    B::Int128,
    B::UInt128,
    B::Float16,
    B::BFloat16,
    B::Float,
    B::Double,
    B::LongDouble,
    B::Float128,
    B::Half,
];

const BITFIELD_INTS: &[B] = &[
    B::Bool,
    B::Char,
    B::SChar,
    B::UChar,
    B::WChar,
    B::Char8,
    B::Char16,
    B::Char32,
    B::Short,
    B::UShort,
    B::Int,
    B::UInt,
    B::Long,
    B::ULong,
    B::LongLong,
    B::ULongLong,
    B::Int128,
    B::UInt128,
];

/// The facts of a field type: `(dc, nontrivial, align_ub)`.
#[derive(Clone, Copy)]
struct TyFacts {
    dc: bool,
    nontrivial: bool,
    align_ub: u64,
}

impl Gen<'_> {
    fn scalar(&mut self) -> B {
        loop {
            let b = self.rng.pick(SCALARS);
            if b != B::Float128 || self.target.has_float128 {
                return b;
            }
        }
    }

    /// A scalar usable as a function parameter or return type (not
    /// `__fp16`).
    fn param(&mut self) -> B {
        loop {
            let b = self.scalar();
            if b != B::Half {
                return b;
            }
        }
    }

    fn bit_int(&mut self) -> Ty {
        let signed = self.rng.chance(50);
        let lo = if signed { 2 } else { 1 };
        let w = if self.rng.chance(70) {
            lo + self.rng.below(66 - lo)
        } else {
            lo + self.rng.below(129 - lo)
        };
        Ty::BitInt(signed, u32::try_from(w).expect("width"))
    }

    fn items_where(&self, f: impl Fn(&Facts) -> bool) -> Vec<usize> {
        self.facts.iter().enumerate().filter(|(_, x)| f(x)).map(|(i, _)| i).collect()
    }

    /// A simple object type usable under a pointer or as a function
    /// parameter.
    fn pointee(&mut self) -> Ty {
        match self.rng.below(6) {
            0 => Ty::B(B::Void),
            1 => Ty::Func(Box::new(Ty::B(B::Void)), vec![]),
            2 => {
                let n = self.rng.below(3);
                let ps = (0..n).map(|_| Ty::B(self.param())).collect();
                let r = Ty::B(self.param());
                Ty::Func(Box::new(r), ps)
            }
            3 => {
                let recs = self.items_where(|f| f.record);
                if recs.is_empty() { Ty::B(B::Int) } else { Ty::Named(self.rng.pick(&recs)) }
            }
            4 => Ty::Cv(
                Box::new(Ty::B(self.scalar())),
                1 + u8::try_from(self.rng.below(3)).expect("cv"),
            ),
            _ => Ty::B(self.scalar()).ptr(),
        }
    }

    /// A non-array, non-reference member type.
    fn element(&mut self, allow_over_aligned: bool) -> (Ty, TyFacts) {
        let plain = TyFacts { dc: true, nontrivial: false, align_ub: 16 };
        match self.rng.below(16) {
            0..=4 => (Ty::B(self.scalar()), plain),
            5 => (self.bit_int(), plain),
            6 => (Ty::Ptr(Box::new(self.pointee())), plain),
            7 => {
                let recs = self.items_where(|f| f.record);
                if recs.is_empty() {
                    return (Ty::B(B::Int).ptr(), plain);
                }
                let c = self.rng.pick(&recs);
                let p = if self.rng.chance(50) {
                    Ty::B(self.scalar())
                } else {
                    let n = self.rng.below(3);
                    Ty::Func(Box::new(Ty::B(B::Int)), (0..n).map(|_| Ty::B(B::Double)).collect())
                };
                (Ty::MemPtr(Box::new(p), c), plain)
            }
            8 => {
                let e =
                    self.rng.pick(&[B::Float, B::Double, B::LongDouble, B::Int, B::Char, B::Short]);
                (Ty::Complex(Box::new(Ty::B(e))), TyFacts { align_ub: 32, ..plain })
            }
            9 => {
                // `_Atomic` does not apply to `_BitInt`.
                let inner =
                    if self.rng.chance(25) { Ty::B(B::Int).ptr() } else { Ty::B(self.scalar()) };
                (Ty::Atomic(Box::new(inner)), TyFacts { align_ub: 32, ..plain })
            }
            10 => (Ty::NullptrT, plain),
            11 => {
                let ty = Ty::Cv(
                    Box::new(Ty::B(self.scalar())),
                    1 + u8::try_from(self.rng.below(3)).expect("cv"),
                );
                // A const member leaves the default constructor deleted.
                let dc = matches!(ty, Ty::Cv(_, 2));
                (ty, TyFacts { dc, ..plain })
            }
            12 => {
                let es = self.items_where(|f| f.enumeration);
                if es.is_empty() {
                    (Ty::B(B::Int), plain)
                } else {
                    (Ty::Named(self.rng.pick(&es)), plain)
                }
            }
            13 => {
                let ok = |f: &Facts| {
                    !f.record && !f.enumeration && (allow_over_aligned || !f.over_aligned)
                };
                let ts = self.items_where(ok);
                if ts.is_empty() {
                    return (Ty::B(B::Int), plain);
                }
                let t = self.rng.pick(&ts);
                (Ty::Named(t), TyFacts { align_ub: self.facts[t].align_ub, ..plain })
            }
            _ => {
                let recs = self.items_where(|f| f.record && !f.fam);
                if recs.is_empty() {
                    return (Ty::B(B::Int), plain);
                }
                let r = self.rng.pick(&recs);
                let f = self.facts[r];
                (Ty::Named(r), TyFacts { dc: f.dc, nontrivial: f.nontrivial, align_ub: f.align_ub })
            }
        }
    }

    /// A member type (possibly an array or a reference).
    fn member(&mut self, union: bool) -> (Ty, TyFacts) {
        match self.rng.below(10) {
            0 | 1 => {
                let (mut t, f) = self.element(false);
                // Arrays of arrays, including zero-length ones.
                let dims = 1 + self.rng.below(2);
                for _ in 0..dims {
                    let n = if self.rng.chance(15) { 0 } else { 1 + self.rng.below(5) };
                    t = t.arr(n);
                }
                (t, f)
            }
            2 if !union && self.rng.chance(40) => {
                let (t, f) = self.element(true);
                let r =
                    if self.rng.chance(50) { Ty::LRef(Box::new(t)) } else { Ty::RRef(Box::new(t)) };
                (r, TyFacts { dc: false, align_ub: 16, ..f })
            }
            _ => self.element(true),
        }
    }

    fn bit_field(&mut self, union: bool) -> (FieldDef, TyFacts) {
        let plain = TyFacts { dc: true, nontrivial: false, align_ub: 16 };
        let enums = self.items_where(|f| f.enumeration);
        let (ty, max_w, wide_ok) = match self.rng.below(10) {
            0 if !enums.is_empty() => (Ty::Named(self.rng.pick(&enums)), 8, false),
            1 => {
                let t = self.bit_int();
                let Ty::BitInt(_, w) = t else { unreachable!() };
                (t, u64::from(w), false)
            }
            _ => {
                let b = self.rng.pick(BITFIELD_INTS);
                let w = self.target.builtin_size_align(b).0 * 8;
                (Ty::B(b), if b == B::Bool { 1 } else { w }, b != B::Bool)
            }
        };
        let width = if self.rng.chance(8) {
            0
        } else if wide_ok && self.rng.chance(if union { 15 } else { 5 }) {
            max_w + 1 + self.rng.below(200 - max_w.min(199))
        } else {
            1 + self.rng.below(max_w)
        };
        let mut f = FieldDef::bits(None, ty, width);
        f.packed = self.rng.chance(10);
        if self.rng.chance(5) {
            f.aligned = Some(1 << self.rng.below(5));
        }
        f.private = self.rng.chance(10);
        (f, plain)
    }

    fn record(&mut self, idx: usize) {
        let union = self.rng.chance(20);
        let tag =
            if union { TagKind::Union } else { self.rng.pick(&[TagKind::Struct, TagKind::Class]) };
        let n_fields = if self.rng.chance(8) { 0 } else { 1 + self.rng.below(7) };
        let mut rec = RecDef::new(&format!("R{idx}"), tag, Vec::new());
        let mut dc = true;
        let mut nontrivial = false;
        let mut ub: u64 = 1;
        for k in 0..n_fields {
            let empties = self.items_where(|f| f.empty);
            let (mut f, tf) = if !empties.is_empty() && self.rng.chance(12) {
                // [[no_unique_address]] empty members, overlapping each
                // other and the next members (C++20 [intro.object]/9).
                let e = self.rng.pick(&empties);
                let ef = self.facts[e];
                let ty = if self.rng.chance(20) {
                    Ty::Named(e).arr(1 + self.rng.below(3))
                } else {
                    Ty::Named(e)
                };
                let f = FieldDef { ty: Some(ty), nua: self.rng.chance(80), ..FieldDef::default() };
                (f, TyFacts { dc: ef.dc, nontrivial: ef.nontrivial, align_ub: ef.align_ub })
            } else if self.rng.chance(30) {
                self.bit_field(union)
            } else {
                let (ty, tf) = self.member(union);
                let mut f = FieldDef { ty: Some(ty), ..FieldDef::default() };
                if self.rng.chance(15) {
                    f.nua = true;
                }
                if self.rng.chance(5) {
                    f.alignas = Some(tf.align_ub.max(16) << self.rng.below(2));
                }
                if self.rng.chance(6) {
                    f.aligned = Some(1 << self.rng.below(7));
                }
                if self.rng.chance(8) {
                    f.packed = true;
                }
                if self.rng.chance(10) {
                    f.private = true;
                }
                (f, tf)
            };
            // Zero-width bit-fields are unnamed; so are some others.
            // Unnamed bit-fields take no attributes.
            let unnamed = f.bits == Some(0) || (f.bits.is_some() && self.rng.chance(15));
            if unnamed {
                f.packed = false;
                f.aligned = None;
                f.private = false;
            } else {
                f.name = Some(format!("f{k}"));
            }
            dc &= tf.dc;
            nontrivial |= tf.nontrivial;
            ub = ub.max(tf.align_ub).max(f.alignas.unwrap_or(0)).max(f.aligned.unwrap_or(0));
            rec.fields.push(f);
        }
        // A flexible array member as the last member of a struct.
        let mut fam = false;
        if !union && !rec.fields.is_empty() && self.rng.chance(8) {
            let (e, tf) = self.element(false);
            if !matches!(e, Ty::Named(i) if self.facts[i].record) {
                let mut f = FieldDef::new(&format!("f{}", rec.fields.len()), Ty::Flex(Box::new(e)));
                f.packed = self.rng.chance(10);
                rec.fields.push(f);
                ub = ub.max(tf.align_ub);
                fam = true;
            }
        }
        if union && nontrivial {
            dc = false;
        }
        if self.rng.chance(10) {
            rec.packed = true;
        }
        if self.rng.chance(8) {
            rec.aligned = Some(if self.rng.chance(20) { 0 } else { 1 << self.rng.below(7) });
            ub = ub.max(rec.aligned.unwrap_or(0)).max(16);
        }
        if self.rng.chance(5) {
            let a = ub.max(16).next_power_of_two() << self.rng.below(2);
            rec.alignas = Some(a);
            ub = ub.max(a);
        }
        if dc && self.rng.chance(15) {
            rec.user_ctor = true;
            nontrivial = true;
        }
        let empty = rec.fields.iter().all(|f| f.bits.is_some() && f.name.is_none());
        self.model.push(Item::Record(rec));
        self.facts.push(Facts {
            record: true,
            empty,
            fam,
            dc,
            nontrivial,
            align_ub: ub.max(16),
            ..Facts::default()
        });
    }

    fn prelude(&mut self) {
        // Enumerations: one unfixed, two fixed.
        for _ in 0..3 {
            let i = self.model.items.len();
            let fixed = if i == 0 { None } else { Some(self.rng.pick(BITFIELD_INTS)) };
            let fixed = fixed.filter(|b| *b != B::Bool || i == 1);
            self.model.push(Item::Enum(EnumDef { name: format!("E{i}"), fixed }));
            self.facts.push(Facts { enumeration: true, align_ub: 16, ..Facts::default() });
        }
        // Typedefs: plain, over-aligned, under-aligned and vectors.
        for k in 0..5 {
            let i = self.model.items.len();
            let base = self.scalar();
            let (ty, aligned, vector_bytes) = match k {
                0 => (Ty::B(base), None, None),
                1 => (Ty::B(base), Some(32), None),
                2 => (Ty::B(B::Int), Some(1 << self.rng.below(3)), None),
                _ => {
                    let e = self.rng.pick(&[
                        B::Char,
                        B::Short,
                        B::Int,
                        B::Float,
                        B::Double,
                        B::LongLong,
                    ]);
                    let size = self.target.builtin_size_align(e).0;
                    let n = 1 << self.rng.below(4);
                    (Ty::B(e), None, Some(size * n))
                }
            };
            let over = aligned.is_some_and(|a| a > 16);
            let ub = aligned.unwrap_or(16).max(16).max(vector_bytes.unwrap_or(0));
            self.model.push(Item::Typedef(TypedefDef {
                name: format!("T{i}"),
                ty,
                aligned,
                vector_bytes,
            }));
            self.facts.push(Facts {
                over_aligned: over || vector_bytes.is_some(),
                align_ub: ub,
                ..Facts::default()
            });
        }
    }
}

fn generate(target: &Target, seed: u64, records: usize) -> Model {
    let mut g = Gen { rng: Rng(seed), target, model: Model::default(), facts: Vec::new() };
    g.prelude();
    for _ in 0..records {
        let i = g.model.items.len();
        g.record(i);
    }
    g.model
}

struct Job {
    triple: &'static str,
    seed: u64,
    model: Model,
}

/// Compares one translation unit; returns `(checked, failures)`.
fn check(job: &Job) -> (usize, Vec<String>) {
    let tgt = target(job.triple).expect("target");
    let src = job.model.cpp();
    let clang = match run_clang(job.triple, &src) {
        Ok(s) => blocks(&s),
        Err(e) => {
            return (
                0,
                vec![format!(
                    "{} seed {:#x}: clang failed:\n{e}\n--- source ---\n{src}",
                    job.triple, job.seed
                )],
            );
        }
    };
    if std::env::var_os("KILN_ABI_DIFF_SHOW").is_some() {
        eprintln!("=== {} seed {:#x} ===\n{src}", job.triple, job.seed);
    }
    let (ast, decls) = job.model.ast(&tgt);
    let mut lay = Layouter::new(&ast, &tgt);
    let mut failures = Vec::new();
    let mut checked = 0;
    for (i, r) in job.model.records() {
        let mut ours = String::new();
        write_record_layout(&mut lay, decls[i], &mut ours);
        let ours = ours.strip_prefix(MARKER).unwrap_or(&ours).to_owned();
        let head = ours.lines().next().unwrap_or_default().to_owned();
        let theirs = clang.get(&head);
        checked += 1;
        if std::env::var_os("KILN_ABI_DIFF_SHOW").is_some() {
            eprintln!("{ours}");
        }
        if theirs == Some(&ours) || XFAIL.iter().any(|(n, _)| *n == r.name) {
            continue;
        }
        failures.push(format!(
            "{} seed {:#x} record {}:\n--- kiln ---\n{ours}--- clang ---\n{}--- source ---\n{}",
            job.triple,
            job.seed,
            r.name,
            theirs.map_or("<no block>", String::as_str),
            record_source(&src, &r.name),
        ));
    }
    (checked, failures)
}

/// The definition of `name` in `src`.
fn record_source(src: &str, name: &str) -> String {
    let mut out = String::new();
    let mut on = false;
    for line in src.lines() {
        if !on && (line.contains(&format!(" {name} {{"))) {
            on = true;
        }
        if on {
            out.push_str(line);
            out.push('\n');
            if line == "};" {
                break;
            }
        }
    }
    out
}

#[test]
fn layout_matches_clang() {
    if !clang_available() {
        eprintln!("skipping: {CLANG} not found");
        return;
    }
    let seed: u64 = std::env::var("KILN_ABI_DIFF_SEED")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0x6b69_6c6e_0050);
    let per_target: usize =
        std::env::var("KILN_ABI_DIFF_RECORDS").ok().and_then(|s| s.parse().ok()).unwrap_or(320);
    let mut jobs = Vec::new();
    for (t, triple) in
        ["x86_64-unknown-linux-gnu", "aarch64-unknown-linux-gnu"].into_iter().enumerate()
    {
        let tgt = target(triple).expect("target");
        for k in 0..per_target.div_ceil(PER_TU) {
            let s = seed.wrapping_mul(1000).wrapping_add((t * 100 + k) as u64);
            jobs.push(Job { triple, seed: s, model: generate(&tgt, s, PER_TU) });
        }
    }
    let results: Vec<(usize, Vec<String>)> = std::thread::scope(|scope| {
        let chunks: Vec<_> = jobs
            .chunks(jobs.len().div_ceil(JOBS))
            .map(|chunk| scope.spawn(move || chunk.iter().map(check).collect::<Vec<_>>()))
            .collect();
        chunks.into_iter().flat_map(|h| h.join().expect("worker")).collect()
    });
    let checked: usize = results.iter().map(|r| r.0).sum();
    let failures: Vec<&String> = results.iter().flat_map(|r| &r.1).collect();
    eprintln!(
        "layout_vs_clang: {checked} records checked, {} mismatches, {} xfail",
        failures.len(),
        XFAIL.len()
    );
    for f in failures.iter().take(usize::MAX) {
        eprintln!("{f}");
    }
    assert!(failures.is_empty(), "{} of {checked} records differ from clang", failures.len());
}
