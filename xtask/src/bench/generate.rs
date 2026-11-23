//! Entries generated at bench time (`generate:`, spec/bench.md "Generated
//! entries"): the committed file is a directive header; the measured source
//! is written into the run's scratch directory, so a multi-megabyte input
//! never lands in the repository.

use std::fmt::Write as _;

/// A `generate:` directive's value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Generator {
    /// `nested-dup <N> <DEPTH> <WIDTH>`: the WP-879 repeated nested
    /// template-id TU (docs/status/perf-history.md "WP-879: frontend throughput on
    /// repeated nested template-ids"). `N` reflection-keyed specializations
    /// of a function and of a class template, each keyed by a template-id
    /// `DEPTH` levels deep whose levels take `WIDTH` arguments and repeat
    /// the previous level twice, so the text doubles per level.
    NestedDup { n: usize, depth: u32, width: usize },
    /// `call-chain <N>`: three expression statements of `N` member calls
    /// each, `x.f().f()...` returning a reference, `p->g()->g()...`, and
    /// `v.h().h()...` returning by value a class with a non-trivial
    /// destructor (a temporary per call). WP-2201: N calls must cost
    /// O(N).
    CallChain { n: usize },
}

/// Bounds that keep a typo from generating gigabytes (the text grows as
/// `N * 2^DEPTH`).
const MAX_N: usize = 100_000;
const MAX_DEPTH: u32 = 10;
const WIDTH: std::ops::RangeInclusive<usize> = 2..=16;
/// The longest `call-chain` (the text grows as `16 * N`).
const MAX_CHAIN: usize = 1_000_000;

impl Generator {
    /// Parses `<generator> <params...>`.
    pub fn parse(value: &str) -> Result<Self, String> {
        let mut words = value.split_whitespace();
        let name = words.next().unwrap_or_default();
        let nums: Vec<&str> = words.collect();
        match name {
            "nested-dup" => {
                let [n, depth, width] = nums[..] else {
                    return Err(format!(
                        "`generate: nested-dup` takes N DEPTH WIDTH, got `{}`",
                        nums.join(" ")
                    ));
                };
                let num = |what: &str, v: &str, ok: &dyn Fn(usize) -> bool| {
                    v.parse::<usize>()
                        .ok()
                        .filter(|&x| ok(x))
                        .ok_or_else(|| format!("bad `nested-dup` {what} `{v}`"))
                };
                let n = num("N (1..=100000)", n, &|x| (1..=MAX_N).contains(&x))?;
                let depth = num("DEPTH (0..=10)", depth, &|x| x <= MAX_DEPTH as usize)?;
                let width = num("WIDTH (2..=16)", width, &|x| WIDTH.contains(&x))?;
                Ok(Self::NestedDup { n, depth: depth as u32, width })
            }
            "call-chain" => {
                let [n] = nums[..] else {
                    return Err(format!(
                        "`generate: call-chain` takes N, got `{}`",
                        nums.join(" ")
                    ));
                };
                let n = n
                    .parse::<usize>()
                    .ok()
                    .filter(|x| (1..=MAX_CHAIN).contains(x))
                    .ok_or_else(|| format!("bad `call-chain` N (1..={MAX_CHAIN}) `{n}`"))?;
                Ok(Self::CallChain { n })
            }
            "" => Err("`generate` requires a generator (nested-dup, call-chain)".into()),
            g => Err(format!("unknown generator `{g}` (nested-dup, call-chain)")),
        }
    }

    /// The generated translation unit. Deterministic: the same directive
    /// always gives the same bytes (they are part of the source hash).
    pub fn source(&self) -> String {
        match *self {
            Self::NestedDup { n, depth, width } => nested_dup(n, depth, width),
            Self::CallChain { n } => call_chain(n),
        }
    }
}

/// The text of [`Generator::NestedDup`]; byte-identical to the Python
/// generator in docs/status/perf-history.md "WP-879" (`gen.py N DEPTH WIDTH 1`).
fn nested_dup(n: usize, depth: u32, width: usize) -> String {
    let mut o = String::new();
    o.push_str(
        "using info = decltype(^^int);\n\
         template <info R> int f() { static int n = 0; return ++n; }\n\
         template <info R> struct S { static int count; };\n\
         template <info R> int S<R>::count = 0;\n\
         namespace lib {\n",
    );
    for k in 0..4 {
        let _ = writeln!(o, "template <class... Ts> struct Node{k} {{}};");
    }
    for i in 0..n {
        let _ = writeln!(o, "struct T{i:05} {{}};");
    }
    o.push_str("}\n");
    let ty = |i: usize| {
        let mut prev = format!("lib::T{i:05}");
        for d in 1..=depth as usize {
            let mut args = vec![prev.clone()];
            for a in 1..width - 1 {
                let j = (i * 7 + d * 13 + a) % n;
                args.push(match a % 5 {
                    0 => format!("lib::T{j:05}"),
                    1 => "int".into(),
                    2 => format!("const lib::T{i:05}*"),
                    3 => "long".into(),
                    _ => format!("lib::T{j:05}&"),
                });
            }
            args.push(prev);
            prev = format!("lib::Node{}<{}>", d % 4, args.join(", "));
        }
        prev
    };
    o.push_str("int (*fs[])() = {\n");
    for i in 0..n {
        let _ = writeln!(o, "  &f<^^{}>,", ty(i));
    }
    o.push_str("};\nint* counts[] = {\n");
    for i in 0..n {
        let _ = writeln!(o, "  &S<^^{}>::count,", ty(i));
    }
    o.push_str("};\n");
    o
}

/// The text of [`Generator::CallChain`]: 50 calls per line.
fn call_chain(n: usize) -> String {
    let mut o = String::from(
        "struct X { X& f(); };\n\
         struct P { P* g(); };\n\
         struct V { ~V(); V h() const; };\n",
    );
    let chain = |o: &mut String, head: &str, call: &str, tail: &str| {
        o.push_str(head);
        for i in 0..n {
            o.push_str(call);
            if i % 50 == 49 {
                o.push('\n');
            }
        }
        o.push_str(tail);
    };
    chain(&mut o, "void a(X x) {\n  x", ".f()", ";\n}\n");
    chain(&mut o, "void b(P* p) {\n  p", "->g()", ";\n}\n");
    chain(&mut o, "void c(V v) {\n  v", ".h()", ";\n}\n");
    o
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_bounds() {
        assert_eq!(
            Generator::parse("nested-dup 200 6 4"),
            Ok(Generator::NestedDup { n: 200, depth: 6, width: 4 })
        );
        assert_eq!(Generator::parse("call-chain 5000"), Ok(Generator::CallChain { n: 5000 }));
        for bad in [
            "",
            "nested",
            "nested-dup 200 6",
            "nested-dup 200 6 4 1",
            "nested-dup 0 6 4",
            "nested-dup 200 11 4",
            "nested-dup 200 6 1",
            "nested-dup x 6 4",
            "call-chain",
            "call-chain 0",
            "call-chain 1000001",
            "call-chain 10 1",
        ] {
            assert!(Generator::parse(bad).is_err(), "{bad}");
        }
    }

    /// The shape of the text: depth 1 at N = 2 written out, and the bench
    /// entry's input (WP-919 keeps it at 1.3 MB) pinned to the Python
    /// generator's bytes.
    #[test]
    fn text() {
        let s = nested_dup(2, 1, 4);
        assert!(s.starts_with("using info = decltype(^^int);\n"), "{s}");
        assert!(s.contains("struct T00001 {};\n}\nint (*fs[])() = {\n"), "{s}");
        assert!(
            s.contains("  &f<^^lib::Node1<lib::T00000, int, const lib::T00000*, lib::T00000>>,\n"),
            "{s}"
        );
        assert!(
            s.contains(
                "  &S<^^lib::Node1<lib::T00001, int, const lib::T00001*, lib::T00001>>::count,\n"
            ),
            "{s}"
        );
        assert!(s.ends_with("::count,\n};\n"), "{s}");
        let bench = Generator::parse("nested-dup 200 6 4").unwrap().source();
        assert_eq!(bench, Generator::parse("nested-dup 200 6 4").unwrap().source());
        // `python3 gen.py 200 6 4 1` (docs/status/perf-history.md "WP-879") writes the same bytes.
        assert_eq!(bench.len(), 1_273_799);
        assert_eq!(super::super::entry::source_hash(bench.as_bytes()), "fnv1a64:852b2d6b5d67e92f");
    }

    /// The `call-chain` text at N = 2.
    #[test]
    fn call_chain_text() {
        let s = call_chain(2);
        assert_eq!(
            s,
            "struct X { X& f(); };\nstruct P { P* g(); };\nstruct V { ~V(); V h() const; };\n\
             void a(X x) {\n  x.f().f();\n}\n\
             void b(P* p) {\n  p->g()->g();\n}\n\
             void c(V v) {\n  v.h().h();\n}\n"
        );
    }
}
