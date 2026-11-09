//! The coverage mapping encoding (WP-1003, spec/codegen.md
//! "Instrumentation"): LLVM 19's `CounterExpressionBuilder` and
//! `CoverageMappingWriter` / `CoverageFilenamesSectionWriter`, format
//! version 6 (`CovMapVersion::Version7`, the header's `i32 6`).
//!
//! The bytes must be the ones clang writes for the same regions: the
//! writer sorts the regions, keeps only the expressions they reach
//! (including the minimizer's duplicate entries for an expression reached
//! twice) and delta-encodes the lines.

use kiln_source::fx::FxHashMap;

/// LLVM `Counter`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub(crate) enum Counter {
    #[default]
    Zero,
    /// `CounterValueReference`: the counter of that index.
    Ref(u32),
    /// The expression of that index.
    Expr(u32),
}

/// LLVM `CounterExpression::ExprKind`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(crate) enum Op {
    Sub = 0,
    Add = 1,
}

/// LLVM `CounterExpression`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(crate) struct CExpr {
    pub op: Op,
    pub lhs: Counter,
    pub rhs: Counter,
}

/// LLVM `CounterExpressionBuilder`: every expression made once.
#[derive(Default, Debug)]
pub(crate) struct ExprBuilder {
    pub exprs: Vec<CExpr>,
    map: FxHashMap<CExpr, u32>,
}

impl ExprBuilder {
    fn get(&mut self, e: CExpr) -> Counter {
        if let Some(&i) = self.map.get(&e) {
            return Counter::Expr(i);
        }
        let i = self.exprs.len() as u32;
        self.exprs.push(e);
        self.map.insert(e, i);
        Counter::Expr(i)
    }

    fn terms(&self, c: Counter, factor: i32, out: &mut Vec<(u32, i32)>) {
        match c {
            Counter::Zero => {}
            Counter::Ref(id) => out.push((id, factor)),
            Counter::Expr(i) => {
                let e = self.exprs[i as usize];
                self.terms(e.lhs, factor, out);
                self.terms(e.rhs, if e.op == Op::Sub { -factor } else { factor }, out);
            }
        }
    }

    /// `CounterExpressionBuilder::simplify`: the terms by counter, the
    /// additions before the subtractions.
    fn simplify(&mut self, c: Counter) -> Counter {
        let mut t = Vec::new();
        self.terms(c, 1, &mut t);
        if t.is_empty() {
            return Counter::Zero;
        }
        t.sort_by_key(|x| x.0);
        let mut merged: Vec<(u32, i32)> = Vec::with_capacity(t.len());
        for (id, f) in t {
            match merged.last_mut() {
                Some(l) if l.0 == id => l.1 += f,
                _ => merged.push((id, f)),
            }
        }
        let mut c = Counter::Zero;
        for &(id, f) in &merged {
            for _ in 0..f.max(0) {
                c = if c == Counter::Zero {
                    Counter::Ref(id)
                } else {
                    self.get(CExpr { op: Op::Add, lhs: c, rhs: Counter::Ref(id) })
                };
            }
        }
        for &(id, f) in &merged {
            for _ in 0..(-f).max(0) {
                c = self.get(CExpr { op: Op::Sub, lhs: c, rhs: Counter::Ref(id) });
            }
        }
        c
    }

    pub(crate) fn add_s(&mut self, a: Counter, b: Counter, simplify: bool) -> Counter {
        let c = self.get(CExpr { op: Op::Add, lhs: a, rhs: b });
        if simplify { self.simplify(c) } else { c }
    }

    pub(crate) fn add(&mut self, a: Counter, b: Counter) -> Counter {
        self.add_s(a, b, true)
    }

    pub(crate) fn sub(&mut self, a: Counter, b: Counter) -> Counter {
        let c = self.get(CExpr { op: Op::Sub, lhs: a, rhs: b });
        self.simplify(c)
    }
}

/// LLVM `CounterMappingRegion::RegionKind`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Kind {
    Code = 0,
    Expansion = 1,
    Skipped = 2,
    Gap = 3,
    Branch = 4,
}

/// LLVM `CounterMappingRegion` (spelling lines and columns, 1-based).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct MapRegion {
    pub kind: Kind,
    pub count: Counter,
    pub false_count: Counter,
    pub file: u32,
    pub expanded: u32,
    pub l1: u32,
    pub c1: u32,
    pub l2: u32,
    pub c2: u32,
}

pub(crate) fn uleb(mut v: u64, out: &mut Vec<u8>) {
    loop {
        let b = (v & 0x7f) as u8;
        v >>= 7;
        if v == 0 {
            out.push(b);
            return;
        }
        out.push(b | 0x80);
    }
}

/// LLVM `CounterExpressionsMinimizer`.
struct Minimizer<'a> {
    exprs: &'a [CExpr],
    used: Vec<CExpr>,
    adjusted: Vec<u32>,
}

impl Minimizer<'_> {
    fn mark(&mut self, c: Counter) {
        if let Counter::Expr(i) = c {
            self.adjusted[i as usize] = 1;
            let e = self.exprs[i as usize];
            self.mark(e.lhs);
            self.mark(e.rhs);
        }
    }

    fn gather(&mut self, c: Counter) {
        let Counter::Expr(i) = c else { return };
        if self.adjusted[i as usize] == 0 {
            return;
        }
        self.adjusted[i as usize] = self.used.len() as u32;
        let e = self.exprs[i as usize];
        self.used.push(e);
        self.gather(e.lhs);
        self.gather(e.rhs);
    }

    fn adjust(&self, c: Counter) -> Counter {
        match c {
            Counter::Expr(i) => Counter::Expr(self.adjusted[i as usize]),
            c => c,
        }
    }
}

fn counter_code(used: &[CExpr], c: Counter) -> u64 {
    match c {
        Counter::Zero => 0,
        Counter::Ref(id) => 1 | (u64::from(id) << 2),
        Counter::Expr(i) => (2 + used[i as usize].op as u64) | (u64::from(i) << 2),
    }
}

/// LLVM `CoverageMappingWriter::write`: `files` is the virtual file
/// mapping (the global filename index of each local file id).
pub(crate) fn write_mapping(
    files: &[u32],
    exprs: &[CExpr],
    regions: &mut [MapRegion],
    out: &mut Vec<u8>,
) {
    regions.sort_by(|a, b| {
        (a.file, a.l1, a.c1, 2 * a.kind as u32).cmp(&(b.file, b.l1, b.c1, 2 * b.kind as u32))
    });
    uleb(files.len() as u64, out);
    for &f in files {
        uleb(u64::from(f), out);
    }
    let mut m = Minimizer { exprs, used: Vec::new(), adjusted: vec![0; exprs.len()] };
    for r in regions.iter() {
        m.mark(r.count);
        m.mark(r.false_count);
    }
    for r in regions.iter() {
        m.gather(r.count);
        m.gather(r.false_count);
    }
    uleb(m.used.len() as u64, out);
    for e in &m.used {
        uleb(counter_code(&m.used, m.adjust(e.lhs)), out);
        uleb(counter_code(&m.used, m.adjust(e.rhs)), out);
    }
    let mut prev_line = 0;
    let mut cur = u32::MAX;
    for (i, r) in regions.iter().enumerate() {
        if r.file != cur {
            let n = regions[i..].iter().take_while(|x| x.file == r.file).count();
            uleb(n as u64, out);
            cur = r.file;
            prev_line = 0;
        }
        let count = m.adjust(r.count);
        let mut c2 = r.c2;
        match r.kind {
            Kind::Code => uleb(counter_code(&m.used, count), out),
            Kind::Gap => {
                uleb(counter_code(&m.used, count), out);
                c2 |= 1 << 31;
            }
            Kind::Expansion => uleb((1 << 2) | (u64::from(r.expanded) << 3), out),
            Kind::Skipped => uleb((Kind::Skipped as u64) << 3, out),
            Kind::Branch => {
                uleb((Kind::Branch as u64) << 3, out);
                uleb(counter_code(&m.used, count), out);
                uleb(counter_code(&m.used, m.adjust(r.false_count)), out);
            }
        }
        uleb(u64::from(r.l1 - prev_line), out);
        uleb(u64::from(r.c1), out);
        uleb(u64::from(r.l2 - r.l1), out);
        uleb(u64::from(c2), out);
        prev_line = r.l1;
    }
}

/// LLVM `CoverageFilenamesSectionWriter::write`, uncompressed (clang
/// compresses with zlib when LLVM is built with it; `llvm-cov` reads
/// both).
pub(crate) fn write_filenames(names: &[String]) -> Vec<u8> {
    let mut body = Vec::new();
    for n in names {
        uleb(n.len() as u64, &mut body);
        body.extend_from_slice(n.as_bytes());
    }
    let mut out = Vec::new();
    uleb(names.len() as u64, &mut out);
    uleb(body.len() as u64, &mut out);
    uleb(0, &mut out);
    out.extend_from_slice(&body);
    out
}

/// LLVM `IndexedInstrProf::ComputeHash` (`MD5Hash`: the low word of the
/// digest, little-endian).
pub(crate) fn md5_hash(b: &[u8]) -> u64 {
    let d = crate::debuginfo::md5::digest(b);
    u64::from_le_bytes([d[0], d[1], d[2], d[3], d[4], d[5], d[6], d[7]])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn region(kind: Kind, count: Counter, l1: u32, c1: u32, l2: u32, c2: u32) -> MapRegion {
        MapRegion { kind, count, false_count: Counter::Zero, file: 0, expanded: 0, l1, c1, l2, c2 }
    }

    /// clang 19's record for `int main(...) {...}` on lines 18-21
    /// (`\01\01\00\01\01\12\1C\03\02`).
    #[test]
    fn single_region() {
        let mut out = Vec::new();
        let mut r = [region(Kind::Code, Counter::Ref(0), 18, 28, 21, 2)];
        write_mapping(&[1], &[], &mut r, &mut out);
        assert_eq!(out, b"\x01\x01\x00\x01\x01\x12\x1C\x03\x02");
    }

    /// `simplify` folds `(c0 + c1) - c1` to `c0` and orders additions
    /// before subtractions; equal expressions are made once.
    #[test]
    fn simplify_and_dedup() {
        let mut b = ExprBuilder::default();
        let s = b.add(Counter::Ref(0), Counter::Ref(1));
        assert_eq!(s, Counter::Expr(0));
        assert_eq!(b.sub(s, Counter::Ref(1)), Counter::Ref(0));
        let t = b.sub(Counter::Ref(2), Counter::Ref(1));
        let u = b.add(t, Counter::Ref(0));
        assert_eq!(b.exprs[match u {
            Counter::Expr(i) => i as usize,
            _ => unreachable!(),
        }]
        .op, Op::Sub);
        assert_eq!(b.sub(Counter::Ref(3), Counter::Ref(3)), Counter::Zero);
        let again = b.add(Counter::Ref(0), Counter::Ref(1));
        assert_eq!(again, s);
    }

    /// The minimizer pushes an expression reached a second time again
    /// (LLVM's `gatherUsed` quirk), unless it got index 0.
    #[test]
    fn minimizer_duplicates() {
        let e = [
            CExpr { op: Op::Add, lhs: Counter::Ref(0), rhs: Counter::Ref(1) },
            CExpr { op: Op::Sub, lhs: Counter::Ref(2), rhs: Counter::Ref(3) },
        ];
        let mut r = [
            region(Kind::Code, Counter::Expr(1), 1, 1, 1, 2),
            region(Kind::Code, Counter::Expr(0), 2, 1, 2, 2),
            region(Kind::Code, Counter::Expr(0), 3, 1, 3, 2),
        ];
        let mut out = Vec::new();
        write_mapping(&[1], &e, &mut r, &mut out);
        // Three expressions: e1, e0, e0 again.
        assert_eq!(out[2], 3);
    }

    #[test]
    fn filenames_blob() {
        let b = write_filenames(&["/d".into(), "a.c".into()]);
        assert_eq!(b, b"\x02\x07\x00\x02/d\x03a.c");
    }
}
