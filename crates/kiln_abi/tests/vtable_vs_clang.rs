//! Differential test: seeded random polymorphic class hierarchies (virtual
//! and repeated bases, overriding, covariant returns, pure, deleted and
//! `consteval` virtual functions, virtual destructors) are laid out by
//! `kiln_abi::vtable` and by `clang++-19 -Xclang -fdump-vtable-layouts`
//! for each supported target; every vtable and construction vtable dump
//! must match byte for byte (spec/abi.md §7, §8.5). Skipped when
//! `clang++-19` is not installed.
//!
//! `KILN_VTABLE_DIFF_SEED` overrides the seed; `KILN_VTABLE_DIFF_CLASSES`
//! the number of classes per target; `KILN_ABI_DIFF_SHOW` prints every
//! generated source.

mod common;

use std::collections::{BTreeSet, HashSet};

use common::{
    CLANG, DtorDef, FieldDef, Item, MethodDef, MethodKind, Model, RecDef, Ty, clang_available,
    is_vtable_header, run_clang_vtables, vtable_blocks,
};
use kiln_abi::target::target;
use kiln_abi::vtable::{VTableError, dump_construction_vtable, dump_vtable_layout, vtt_vtables};
use kiln_ast::decl::TagKind;
use kiln_ast::types::BuiltinType as B;

/// Classes per generated translation unit.
const PER_TU: usize = 25;
/// Concurrent clang processes.
const JOBS: usize = 8;
/// The largest subobject graph a generated class may have.
const MAX_NODES: usize = 48;

/// Known divergences: `(class name, reason)`. Empty: none are known.
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
    fn index(&mut self, n: usize) -> usize {
        usize::try_from(self.below(n as u64)).expect("index")
    }
    fn chance(&mut self, percent: u64) -> bool {
        self.below(100) < percent
    }
    fn pick<T: Clone>(&mut self, xs: &[T]) -> T {
        xs[self.index(xs.len())].clone()
    }
}

struct Gen {
    rng: Rng,
    model: Model,
    next_name: usize,
}

/// The descendants of each node of a subobject graph.
fn descendants(graph: &[(usize, Vec<usize>)]) -> Vec<BTreeSet<usize>> {
    fn go(
        graph: &[(usize, Vec<usize>)],
        n: usize,
        memo: &mut Vec<Option<BTreeSet<usize>>>,
    ) -> BTreeSet<usize> {
        if let Some(s) = &memo[n] {
            return s.clone();
        }
        let mut s = BTreeSet::new();
        for &c in &graph[n].1 {
            s.insert(c);
            s.extend(go(graph, c, memo));
        }
        memo[n] = Some(s.clone());
        s
    }
    let mut memo = vec![None; graph.len()];
    (0..graph.len()).map(|n| go(graph, n, &mut memo)).collect()
}

impl Gen {
    fn fresh_name(&mut self) -> String {
        self.next_name += 1;
        format!("f{}", self.next_name)
    }

    /// The number of subobjects of class `c` in an object of class
    /// `r` (item `idx`).
    fn count_in(&self, idx: usize, r: &RecDef, c: usize) -> usize {
        if idx == c {
            return 1;
        }
        self.model
            .subobject_graph_of(idx, r, MAX_NODES * 4)
            .map_or(usize::MAX, |g| g.iter().filter(|n| n.0 == c).count())
    }

    /// Every function `(class, method)` overrides, directly or not.
    fn all_overridden(&self, direct: &[(usize, usize)]) -> Vec<(usize, usize)> {
        let mut out: Vec<(usize, usize)> = Vec::new();
        let mut stack = direct.to_vec();
        while let Some((c, k)) = stack.pop() {
            if out.contains(&(c, k)) {
                continue;
            }
            out.push((c, k));
            let r = self.model.rec(c);
            stack.extend(self.model.overridden(&r.bases, &r.methods[k]));
        }
        out
    }

    /// A return type for an overrider in `r` (item `idx`) of the
    /// functions `direct`: the same type, or a covariant class pointer
    /// or reference ([class.virtual]/8) whose class has a unique
    /// subobject of every overridden function's return class.
    fn covariant_return(
        &mut self,
        idx: usize,
        r: &RecDef,
        direct: &[(usize, usize)],
    ) -> Option<Ty> {
        let all = self.all_overridden(direct);
        let rets: Vec<Ty> =
            all.iter().map(|&(c, k)| self.model.rec(c).methods[k].ret.clone()).collect();
        let first = rets[0].clone();
        let class_of = |t: &Ty| match t {
            Ty::Ptr(p) | Ty::LRef(p) => match **p {
                Ty::Named(c) => Some(c),
                _ => None,
            },
            _ => None,
        };
        let Some(_) = class_of(&first) else {
            return rets.iter().all(|t| *t == first).then_some(first);
        };
        let is_ptr = matches!(first, Ty::Ptr(_));
        let mut classes = Vec::new();
        for t in &rets {
            if matches!(t, Ty::Ptr(_)) != is_ptr {
                return None;
            }
            classes.push(class_of(t)?);
        }
        let mut candidates: Vec<usize> = Vec::new();
        for c2 in 0..=idx {
            let ok = classes.iter().all(|&c| {
                if c2 == c {
                    return true;
                }
                if c2 == idx {
                    self.count_in(idx, r, c) == 1
                } else {
                    self.count_in(c2, self.model.rec(c2), c) == 1
                }
            });
            if ok {
                candidates.push(c2);
            }
        }
        if candidates.is_empty() {
            return None;
        }
        let c2 = self.rng.pick(&candidates);
        let named = Box::new(Ty::Named(c2));
        Some(if is_ptr { Ty::Ptr(named) } else { Ty::LRef(named) })
    }

    /// Completes `m` as a member of `r` (item `idx`): if it overrides,
    /// its kind and return type follow the overridden functions (`None`
    /// if they cannot); otherwise they are random.
    fn settle(&mut self, idx: usize, r: &RecDef, mut m: MethodDef) -> Option<MethodDef> {
        if r.methods.iter().any(|o| o.same_signature(&m)) {
            return None;
        }
        let direct = self.model.overridden(&r.bases, &m);
        if direct.is_empty() {
            // [dcl.constexpr]/3 (clang 19): no constexpr member functions
            // in a class with virtual bases.
            return (m.kind != MethodKind::Consteval || !r.has_vbases).then_some(m);
        }
        let kinds: HashSet<MethodKind> =
            direct.iter().map(|&(c, k)| self.model.rec(c).methods[k].kind).collect();
        if kinds.len() != 1 {
            return None;
        }
        m.kind = *kinds.iter().next().expect("kind");
        if m.kind == MethodKind::Consteval && r.has_vbases {
            return None;
        }
        if m.kind != MethodKind::Normal {
            m.pure = false;
        }
        m.ret = self.covariant_return(idx, r, &direct)?;
        Some(m)
    }

    fn params(&mut self) -> Vec<Ty> {
        self.rng.pick(&[
            vec![],
            vec![],
            vec![Ty::B(B::Int)],
            vec![Ty::B(B::Double)],
            vec![Ty::B(B::Char), Ty::B(B::Int)],
            vec![Ty::B(B::Int).ptr()],
        ])
    }

    /// A new virtual (or, rarely, non-virtual) function.
    fn new_method(&mut self, idx: usize, inherited: &[MethodDef]) -> MethodDef {
        let name = if !inherited.is_empty() && self.rng.chance(20) {
            self.rng.pick(inherited).name
        } else {
            self.fresh_name()
        };
        let kind = match self.rng.below(100) {
            0..4 => MethodKind::Consteval,
            4..8 => MethodKind::Deleted,
            _ => MethodKind::Normal,
        };
        let ret = match (kind, self.rng.below(100)) {
            (MethodKind::Consteval | MethodKind::Deleted, 0..50) | (_, 0..40) => Ty::B(B::Void),
            (MethodKind::Consteval | MethodKind::Deleted, _) | (_, 40..60) => Ty::B(B::Int),
            (_, 60..85) => Ty::Named(self.rng.index(idx + 1)).ptr(),
            _ => Ty::LRef(Box::new(Ty::Named(self.rng.index(idx + 1)))),
        };
        let virt = self.rng.chance(92);
        MethodDef {
            name,
            ret,
            params: self.params(),
            is_const: self.rng.chance(20),
            virtual_written: virt,
            pure: virt && kind == MethodKind::Normal && self.rng.chance(12),
            kind,
            is_virtual: false,
        }
    }

    /// The virtual functions of every base subobject of `r`, one per
    /// signature.
    fn inherited(&self, idx: usize, r: &RecDef) -> Vec<MethodDef> {
        let mut out: Vec<MethodDef> = Vec::new();
        let Some(graph) = self.model.subobject_graph_of(idx, r, MAX_NODES) else {
            return out;
        };
        for &(c, _) in graph.iter().skip(1) {
            for m in &self.model.rec(c).methods {
                if m.is_virtual && !out.iter().any(|o| o.same_signature(m)) {
                    out.push(m.clone());
                }
            }
        }
        out
    }

    fn bases(&mut self, idx: usize, r: &mut RecDef) {
        if idx == 0 || !self.rng.chance(80) {
            return;
        }
        let n = 1 + self.rng.below(3);
        for _ in 0..n {
            let c = if self.rng.chance(60) {
                idx - 1 - self.rng.index(idx.min(6))
            } else {
                self.rng.index(idx)
            };
            if r.bases.contains(&c) {
                continue;
            }
            r.bases.push(c);
            if self.rng.chance(35) {
                r.virtual_bases.push(c);
            }
        }
    }

    /// Signatures with more than one final overrider in some subobject
    /// of record `idx` (ill-formed, [class.virtual]/2).
    fn ambiguous_overriders(model: &Model, idx: usize) -> Option<Vec<MethodDef>> {
        let graph = model.subobject_graph(idx, MAX_NODES)?;
        let desc = descendants(&graph);
        let mut out: Vec<MethodDef> = Vec::new();
        for (n, &(k, _)) in graph.iter().enumerate() {
            for m in &model.rec(k).methods {
                if !m.is_virtual {
                    continue;
                }
                let cands: Vec<usize> = (0..graph.len())
                    .filter(|&a| a == n || desc[a].contains(&n))
                    .filter(|&a| {
                        model
                            .rec(graph[a].0)
                            .methods
                            .iter()
                            .any(|o| o.is_virtual && o.same_signature(m))
                    })
                    .collect();
                let finals = cands
                    .iter()
                    .filter(|&&a| !cands.iter().any(|&b| b != a && desc[b].contains(&a)))
                    .count();
                if finals > 1 && !out.iter().any(|o| o.same_signature(m)) {
                    out.push(m.clone());
                }
            }
        }
        Some(out)
    }

    fn try_class(&mut self, idx: usize) -> Option<RecDef> {
        let tag = self.rng.pick(&[TagKind::Struct, TagKind::Class]);
        let mut r = RecDef::new(&format!("R{idx}"), tag, Vec::new());
        r.out_of_line = true;
        r.user_ctor = true;
        self.bases(idx, &mut r);
        self.model.subobject_graph_of(idx, &r, MAX_NODES)?;
        self.model.finish(&mut r);
        for k in 0..self.rng.below(3) {
            let ty = self.rng.pick(&[B::Int, B::Char, B::Double, B::Short]);
            r.fields.push(FieldDef::new(&format!("x{k}"), Ty::B(ty)));
        }
        let plain = self.rng.chance(8);
        let inherited = self.inherited(idx, &r);
        if !plain {
            for _ in 0..self.rng.below(5) {
                let m = if !inherited.is_empty() && self.rng.chance(55) {
                    let mut m = self.rng.pick(&inherited);
                    m.virtual_written = self.rng.chance(50);
                    m.pure = self.rng.chance(10);
                    m
                } else {
                    self.new_method(idx, &inherited)
                };
                if let Some(m) = self.settle(idx, &r, m) {
                    r.methods.push(m);
                    self.model.finish(&mut r);
                }
            }
        }
        if self.rng.chance(35) {
            let virt = !plain && self.rng.chance(70);
            r.dtor = Some(DtorDef {
                virtual_written: virt,
                pure: virt && self.rng.chance(10),
                pos: self.rng.index(r.methods.len() + 1),
            });
        }
        self.model.finish(&mut r);
        // clang emits the vtable (and the VTT, and so dumps the
        // construction vtables) of a class with virtual bases only with
        // its key function or its complete-object constructor, which an
        // abstract class lacks: give such classes a key function.
        let key = r.methods.iter().any(|m| m.is_virtual && !m.pure && m.kind == MethodKind::Normal)
            || r.dtor.is_some_and(|d| !d.pure && r.dtor_virtual);
        if r.has_vbases && !key {
            match &mut r.dtor {
                Some(d) if d.pure => {
                    let mut m = self.new_method(idx, &[]);
                    m.kind = MethodKind::Normal;
                    m.virtual_written = true;
                    m.pure = false;
                    r.methods.push(m);
                }
                Some(d) => d.virtual_written = true,
                None => {
                    let pos = self.rng.index(r.methods.len() + 1);
                    r.dtor = Some(DtorDef { virtual_written: true, pure: false, pos });
                }
            }
            self.model.finish(&mut r);
        }
        // Override what has several final overriders here.
        for _ in 0..2 {
            let mut probe = self.model.clone();
            probe.push(Item::Record(r.clone()));
            let ambiguous = Self::ambiguous_overriders(&probe, idx)?;
            if ambiguous.is_empty() {
                return Some(r);
            }
            for mut m in ambiguous {
                m.virtual_written = self.rng.chance(50);
                m.pure = false;
                let m = self.settle(idx, &r, m)?;
                r.methods.push(m);
                self.model.finish(&mut r);
            }
        }
        None
    }

    fn class(&mut self, idx: usize) {
        for _ in 0..16 {
            if let Some(r) = self.try_class(idx) {
                self.model.push(Item::Record(r));
                return;
            }
        }
        let mut r = RecDef::new(&format!("R{idx}"), TagKind::Struct, Vec::new());
        r.out_of_line = true;
        r.user_ctor = true;
        r.methods.push(self.new_method(idx, &[]));
        self.model.push(Item::Record(r));
    }
}

fn generate(seed: u64, classes: usize) -> Model {
    let mut g = Gen { rng: Rng(seed), model: Model::default(), next_name: 0 };
    for i in 0..classes {
        g.class(i);
    }
    g.model
}

struct Job {
    triple: &'static str,
    seed: u64,
    model: Model,
}

/// Compares one translation unit; returns `(vtables checked, failures)`.
fn check(job: &Job) -> (usize, Vec<String>) {
    let tgt = target(job.triple).expect("target");
    let src = job.model.cpp();
    let ctx = |what: &str| format!("{} seed {:#x}: {what}", job.triple, job.seed);
    let clang = match run_clang_vtables(job.triple, &src) {
        Ok(s) => vtable_blocks(&s),
        Err(e) => return (0, vec![ctx(&format!("clang failed:\n{e}\n--- source ---\n{src}"))]),
    };
    let show = std::env::var_os("KILN_ABI_DIFF_SHOW").is_some();
    if show {
        eprintln!("=== {} seed {:#x} ===\n{src}", job.triple, job.seed);
    }
    let (ast, decls) = job.model.ast(&tgt);
    let mut failures = Vec::new();
    let mut checked = 0;
    let mut ours_heads: HashSet<String> = HashSet::new();
    let mut compare = |name: &str, ours: String, failures: &mut Vec<String>| {
        let head = ours.lines().next().unwrap_or_default().to_owned();
        let theirs = clang.get(&head);
        ours_heads.insert(head);
        if show {
            eprint!("{ours}");
        }
        if theirs == Some(&ours) || XFAIL.iter().any(|(n, _)| *n == name) {
            return;
        }
        failures.push(ctx(&format!(
            "class {name}:\n--- kiln ---\n{ours}--- clang ---\n{}",
            theirs.map_or("<no block>", String::as_str),
        )));
    };
    for (i, r) in job.model.records() {
        let d = decls[i];
        match dump_vtable_layout(&ast, d, &tgt) {
            Ok(text) => {
                checked += 1;
                compare(&r.name, text, &mut failures);
            }
            Err(VTableError::NotDynamic) if !r.dynamic() => continue,
            Err(e) => {
                failures.push(ctx(&format!("class {}: kiln: {e:?}", r.name)));
                continue;
            }
        }
        let subs = match vtt_vtables(&ast, d, &tgt) {
            Ok(s) => s,
            Err(e) => {
                failures.push(ctx(&format!("class {}: kiln VTT: {e:?}", r.name)));
                continue;
            }
        };
        for &sub in subs.iter().skip(1) {
            match dump_construction_vtable(&ast, d, sub, &tgt) {
                Ok(text) => {
                    checked += 1;
                    compare(&r.name, text, &mut failures);
                }
                Err(e) => {
                    failures.push(ctx(&format!("class {}: kiln construction: {e:?}", r.name)));
                }
            }
        }
    }
    let mut missing: Vec<&String> =
        clang.keys().filter(|h| is_vtable_header(h) && !ours_heads.contains(*h)).collect();
    missing.sort();
    for h in missing {
        failures.push(ctx(&format!("kiln did not produce {h}\n{}", clang[h])));
    }
    if !failures.is_empty() {
        failures.push(ctx(&format!("--- source ---\n{src}")));
    }
    (checked, failures)
}

#[test]
fn vtables_match_clang() {
    if !clang_available() {
        eprintln!("skipping: {CLANG} not found");
        return;
    }
    let seed: u64 = std::env::var("KILN_VTABLE_DIFF_SEED")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0x6b69_6c6e_0078);
    let per_target: usize =
        std::env::var("KILN_VTABLE_DIFF_CLASSES").ok().and_then(|s| s.parse().ok()).unwrap_or(500);
    let mut jobs = Vec::new();
    for (t, triple) in
        ["x86_64-unknown-linux-gnu", "aarch64-unknown-linux-gnu"].into_iter().enumerate()
    {
        for k in 0..per_target.div_ceil(PER_TU) {
            let s = seed.wrapping_mul(1000).wrapping_add((t * 100 + k) as u64);
            jobs.push(Job { triple, seed: s, model: generate(s, PER_TU) });
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
    let classes = jobs.len() * PER_TU;
    eprintln!(
        "vtable_vs_clang: {classes} classes, {checked} vtables checked, {} failures, {} xfail",
        failures.len(),
        XFAIL.len()
    );
    for f in &failures {
        eprintln!("{f}");
    }
    assert!(failures.is_empty(), "{} vtable dumps differ from clang", failures.len());
}
