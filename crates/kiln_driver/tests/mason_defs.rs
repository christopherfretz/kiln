//! WP-1581: `.mason.meta` kind 2, one row per definition symbol (mortar's
//! odr.md O-14, O-15; spec/codegen.md §5.5 "Kind 2"), end to end through
//! kiln++. The decoders are `mason_common`'s, written from mortar's spec.
//!
//! - a golden row per definition kind (T-O-14), and the symbols that get
//!   none: owned symbols, ordinary strong definitions, `gnu_inline` (N7),
//!   weak (N5), internal linkage, a lambda in a declarator (X15), C (N21);
//! - N9, N22 and K13 across two units; the structor variants;
//! - the decided forms: `asm` labels, `alias`/`ifunc`, a coroutine's ramp,
//!   `always_inline`;
//! - two library-heavy units per standard library agree;
//! - the flag off changes nothing; two directories give the same bytes;
//!   MT-41's size gates with kind 2 at `default`;
//! - kind 5 (WP-1581): no row for a C-linkage function whose type has a
//!   variable array bound.

mod mason_common;

use std::path::Path;

use mason_common::*;

const VAGUE: u16 = 1;
const IMPLICIT_INST: u16 = 2;
const EXPLICIT_INST: u16 = 4;
const EXPLICIT_SPEC: u16 = 8;
const USES_INTERNAL: u16 = 16;
const PARTIAL: u16 = 32;
const SYSTEM: u16 = 256;
const FUNCTION: u16 = 1;
const VARIABLE: u16 = 2;

/// One kind 2 row.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Def {
    key: u64,
    hash: u128,
    class: u16,
    flags: u16,
    file: String,
    line: u32,
}

/// The kind 2 rows of the unit in `b`, `None` without a kind 2 block,
/// after checking the framing, the block and every row (O-15: `name` 0,
/// `module` 0, reserved 0, a hash, no flag kiln does not write,
/// sorted by key, one row per key).
fn def_rows(b: &[u8]) -> Option<Vec<Def>> {
    let summary = summary_rows(b);
    let at = block_at(b, 2)?;
    assert_eq!((u16_at(b, at), u16_at(b, at + 2)), (2, 1), "kind 2, version 1");
    let (block_size, row_size, rows) =
        (u32_at(b, at + 4) as usize, u32_at(b, at + 8) as usize, u32_at(b, at + 12) as usize);
    assert_eq!(row_size, 48);
    assert!(rows > 0, "no block without rows");
    assert!(16 + rows * 48 <= block_size && at + block_size <= u32_at(b, 20) as usize);
    assert_eq!(value(&summary, "odr.scheme"), Some("kiln-odr-1.d5"));
    let out: Vec<Def> = (0..rows)
        .map(|i| {
            let r = at + 16 + i * 48;
            assert_eq!(u32_at(b, r + 24), 0, "name: anchored rows only");
            assert_eq!(u32_at(b, r + 32), 0, "module");
            assert_eq!(u32_at(b, r + 44), 0, "reserved");
            let d = Def {
                key: u64_at(b, r),
                hash: u128::from_le_bytes(b[r + 8..r + 24].try_into().unwrap()),
                class: u16_at(b, r + 28),
                flags: u16_at(b, r + 30),
                file: string_at(b, u32_at(b, r + 36)),
                line: u32_at(b, r + 40),
            };
            assert!(d.class == FUNCTION || d.class == VARIABLE, "{d:?}");
            assert_ne!(d.hash, 0, "{d:?}");
            assert_eq!(
                d.flags
                    & !(VAGUE
                        | IMPLICIT_INST
                        | EXPLICIT_INST
                        | EXPLICIT_SPEC
                        | USES_INTERNAL
                        | PARTIAL
                        | SYSTEM),
                0,
                "{d:?}"
            );
            d
        })
        .collect();
    for w in out.windows(2) {
        assert!(w[0].key < w[1].key, "sorted by key, one row per key: {w:?}");
    }
    Some(out)
}

/// The IR of `src` compiled with `-fmason-metadata` and `args`.
fn ir_of(dir: &Path, file: &str, src: &str, args: &[&str]) -> String {
    std::fs::write(dir.join(file), src).unwrap();
    let mut a = vec!["-fmason-metadata", "-S", "-emit-llvm", "-o", "-"];
    a.extend(args);
    a.push(file);
    ok(dir, &a)
}

/// The kind 2 rows of `src` compiled with `args` (through the IR).
fn defs_of(dir: &Path, file: &str, src: &str, args: &[&str]) -> Option<Vec<Def>> {
    def_rows(&ir_bytes(meta_line(&ir_of(dir, file, src, args)).expect("the global")))
}

/// The row of `symbol`, if any.
fn row<'r>(rows: &'r [Def], symbol: &str) -> Option<&'r Def> {
    rows.iter().find(|d| d.key == fnv(symbol))
}

/// The symbols IR `ir` defines (functions, variables, aliases).
fn defined(ir: &str) -> Vec<String> {
    ir.lines()
        .filter_map(|l| {
            let rest = if l.starts_with("define ") {
                l.split_once(" @")?.1
            } else if let Some(r) = l.strip_prefix('@') {
                let (_, init) = r.split_once(" = ")?;
                if init.starts_with("external ") || init.contains("declare ") {
                    return None;
                }
                r
            } else {
                return None;
            };
            let rest = rest.trim_start_matches('"');
            Some(rest[..rest.find(['(', ' ', '"']).unwrap_or(rest.len())].to_owned())
        })
        .collect()
}

/// One definition of each kind of O-14, and definitions with no row.
const KINDS: &str = r"inline int inl(int a) { return a; }
inline int ivar = 3;
template <class T> T tf(T t) { return t; }
template <class T> struct Box { T t; int get() const; static int sm; void inl_m() {} template <class U> U mt(U u) { return u; } };
template <class T> int Box<T>::get() const { return 1; }
template <class T> int Box<T>::sm = 5;
template <class T> constexpr T vt = T(2);
template <> long tf<long>(long) { return 9; }
template <> inline short tf<short>(short) { return 8; }
template struct Box<char>;
template char tf<char>(char);
template <> struct Box<double> { int get() const; };
int Box<double>::get() const { return 4; }
template <> int Box<float>::get() const { return 5; }
struct A { static constexpr int n = 3; };
struct D { D() {} ~D() {} virtual void v() {} };
struct Imp { D d; };
extern inline __attribute__((gnu_inline)) int gi() { return 1; }
__attribute__((weak)) int wk() { return 1; }
static inline int st() { return 1; }
namespace { inline int an() { return 1; } }
int strong() { return 1; }
int gs = 1;
inline auto gl = [](auto x) { return x; };
inline thread_local int tl = 1;
inline int sl() { static int s = 1; return s; }
inline int (*fp)(int) = [](int x) { return x; };
int use() {
  Box<int> b; Imp i; Box<double> bd; Box<float> bf;
  return inl(1) + ivar + tf(1) + b.get() + Box<int>::sm + *&vt<int> + tf(2L) + tf((short)1) + bd.get() + bf.get()
       + *&A::n + gi() + wk() + st() + an() + strong() + b.mt(1) + gl(1) + tl + sl() + fp(1) + (b.inl_m(), 0);
}
";

const F: u16 = FUNCTION;
const V: u16 = VARIABLE;
/// `VAGUE`, the flags of a plain inline definition.
const IN: u16 = VAGUE;

/// T-O-14: a row per definition kind, each symbol the object defines
/// for one, and nothing else.
#[test]
fn definition_rows_golden() {
    let d = scratch("defs-golden");
    let ir = ir_of(&d, "k.cpp", KINDS, &["-std=c++20"]);
    let rows = def_rows(&ir_bytes(meta_line(&ir).unwrap())).unwrap();
    #[rustfmt::skip]
    let golden: &[(&str, u16, u16, u32)] = &[
        // Inline function and variable; a thread_local inline variable.
        ("_Z3inli", F, IN, 1),
        ("ivar", V, IN, 2),
        ("tl", V, IN, 25),
        // Implicit instantiations: function, member function, static data
        // member, member template, variable template, an inline member.
        ("_Z2tfIiET_S0_", F, IN | IMPLICIT_INST, 3),
        ("_ZNK3BoxIiE3getEv", F, IN | IMPLICIT_INST, 5),
        ("_ZN3BoxIiE2smE", V, IN | IMPLICIT_INST, 4),
        ("_ZN3BoxIiE2mtIiEET_S2_", F, IN | IMPLICIT_INST, 4),
        ("_Z2vtIiE", V, IN | IMPLICIT_INST, 7),
        ("_ZN3BoxIiE5inl_mEv", F, IN | IMPLICIT_INST, 4),
        // Explicit instantiation definitions: a function and a class's
        // members (weak_odr: vague). The place is the template's.
        ("_Z2tfIcET_S0_", F, IN | EXPLICIT_INST, 3),
        ("_ZNK3BoxIcE3getEv", F, IN | EXPLICIT_INST, 5),
        ("_ZN3BoxIcE2smE", V, IN | EXPLICIT_INST, 4),
        ("_ZN3BoxIcE5inl_mEv", F, IN | EXPLICIT_INST, 4),
        // Explicit specializations: strong (K13) and inline; a member of
        // an explicitly specialized class (decided: EXPLICIT_SPEC, since
        // [temp.expl.spec]/5 defines it as the specialization's member,
        // never instantiated); an explicitly specialized member.
        ("_Z2tfIlET_S0_", F, EXPLICIT_SPEC, 8),
        ("_Z2tfIsET_S0_", F, IN | EXPLICIT_SPEC, 9),
        ("_ZNK3BoxIdE3getEv", F, EXPLICIT_SPEC, 13),
        ("_ZNK3BoxIfE3getEv", F, EXPLICIT_SPEC, 14),
        // A static constexpr data member (inline since C++17).
        ("_ZN1A1nE", V, IN, 15),
        // Inline members and structors; implicitly defined structors
        // (the place is the class).
        ("_ZN1DC2Ev", F, IN, 16),
        ("_ZN1DD2Ev", F, IN, 16),
        ("_ZN1D1vEv", F, IN, 16),
        ("_ZN3ImpC2Ev", F, IN, 17),
        ("_ZN3ImpD2Ev", F, IN, 17),
        // A closure's `operator()` (an inline variable's lambda) and a
        // generic lambda's specialization; the variables themselves (a
        // lambda initializer is hashed since K-3d).
        ("_ZNK2fpMUliE_clEi", F, IN, 27),
        ("_ZNK2glMUlT_E_clIiEEDaS0_", F, IN | IMPLICIT_INST, 24),
        ("fp", V, IN, 27),
        ("gl", V, IN, 24),
        // A function with a static local (the local and its guard: none).
        ("_Z2slv", F, IN, 26),
    ];
    let mut wrong = Vec::new();
    for &(sym, class, flags, line) in golden {
        let r = row(&rows, sym).unwrap_or_else(|| panic!("no row for {sym}"));
        let want = Def { key: fnv(sym), hash: r.hash, class, flags, file: "k.cpp".into(), line };
        if r != &want {
            wrong.push(format!("{sym}: got {r:?}"));
        }
    }
    assert!(wrong.is_empty(), "{wrong:#?}");
    assert_eq!(rows.len(), golden.len(), "{rows:#?}");
    let defs = defined(&ir);
    for sym in [
        "_Z2wkv",     // N5: weak, not vague
        "_Z6strongv", // ordinary strong definitions
        "gs",
        "_Z3usev",
        "_ZL2stv", // internal linkage
        "_ZN12_GLOBAL__N_12anEv",
        "_ZN2fpMUliE_8__invokeEi", // the closure's static invoker
        "_ZZ2slvE1s",              // owned: a static local, vtable, typeinfo
        "_ZTV1D",
        "_ZTI1D",
        "_ZTS1D",
    ] {
        assert!(defs.iter().any(|s| s == sym), "{sym} is defined");
        assert_eq!(row(&rows, sym), None, "{sym}");
    }
    // N7: a `gnu_inline` definition is `available_externally` or absent.
    assert_eq!(row(&rows, "_Z2giv"), None);
}

/// The same symbol at `-O2`: rows are the front end's, taken before the
/// optimizer runs, so `-O2` writes the same rows.
#[test]
fn same_rows_at_o0_and_o2() {
    let d = scratch("defs-o2");
    let o0 = defs_of(&d, "k.cpp", KINDS, &["-std=c++20", "-O0"]);
    assert_eq!(defs_of(&d, "k.cpp", KINDS, &["-std=c++20", "-O2"]), o0);
}

/// Definitions with no row: a lambda in a declarator (X15), a C unit
/// (N21), a unit without rows (no block, no `odr.scheme`).
#[test]
fn units_and_definitions_without_rows() {
    let d = scratch("defs-none");
    let x15 = "inline int g(int a = [] { return 1; }()) { return a; }\n\
               template <class T> inline int h(T, decltype([] {}) = {}) { return 2; }\n\
               inline int k() { return 3; }\n\
               template <class T> inline auto r(T) -> decltype([] { return 4; }) { return {}; }\n\
               template <class T> inline int q(T) requires ([] { return true; }()) { return 5; }\n\
               int use() { return g() + h(1) + k() + r(1)() + q(1); }\n";
    let rows = defs_of(&d, "x.cpp", x15, &["-std=c++20"]).unwrap();
    // Only `k` and the closure members of `r`'s return type (an ordinary
    // closure's call operator: its own row).
    assert!(rows.iter().all(|r| r.line == 3 || r.line == 4), "{rows:?}");
    assert_eq!(rows.iter().filter(|r| r.line == 3).count(), 1, "{rows:?}");
    assert!(row(&rows, "_Z1kv").is_some());
    // C: inline definitions, but no kind 2 and no `odr.scheme` (N21).
    let c = "inline int ci(void) { return 1; }\nextern inline int ce(void) { return 2; }\nint cu(void) { return ci() + ce(); }\n";
    let ir = ir_of(&d, "c.c", c, &["-std=c17"]);
    let unit = ir_bytes(meta_line(&ir).unwrap());
    assert_eq!(def_rows(&unit), None);
    assert_eq!(value(&summary_rows(&unit), "odr.scheme"), None);
    // C++ without a vague definition: no block, no `odr.scheme`.
    let ir = ir_of(&d, "s.cpp", "int s() { return 1; }\n", &[]);
    let unit = ir_bytes(meta_line(&ir).unwrap());
    assert_eq!(def_rows(&unit), None);
    assert_eq!(value(&summary_rows(&unit), "odr.scheme"), None);
    // Every level writes kind 2, from `min` (mortar's K-1 level table).
    for l in ["min", "default", "detail", "all"] {
        let rows = defs_of(&d, "x.cpp", x15, &["-std=c++20", &format!("-fmason-metadata={l}")]);
        assert_eq!(rows.map(|r| r.iter().filter(|r| r.line == 3).count()), Some(1), "{l}");
    }
}

/// The objects of `units` (name, source), compiled with `args` in `d`.
fn objects(d: &Path, units: &[(&str, &str)], args: &[&str]) -> Vec<Vec<u8>> {
    units
        .iter()
        .map(|(name, src)| {
            std::fs::write(d.join(name), src).unwrap();
            let out = format!("{name}.o");
            let mut a = vec!["-fmason-metadata", "-c", "-o", &out];
            a.extend(args);
            a.push(name);
            ok(d, &a);
            container(&std::fs::read(d.join(&out)).unwrap())
        })
        .collect()
}

/// N9 (explicit instantiation in one unit, implicit in the other), N22
/// (the strong C++14 definition of a `static constexpr` member against
/// C++17's inline one): one key, equal hashes, the flags tell them
/// apart. K13 (an explicit specialization against an implicit
/// instantiation): one key, the flags and the hashes differ.
#[test]
fn n9_n22_k13_across_units() {
    let dir = scratch("defs-pairs");
    let bx = "template <class T> struct Box { T t; int get() const { return 1; } };\n";
    let n9 = objects(
        &dir,
        &[
            ("n9a.cpp", &format!("{bx}template struct Box<char>;\n")),
            ("n9b.cpp", &format!("{bx}int b() {{ Box<char> x; return x.get(); }}\n")),
        ],
        &[],
    );
    let n9a = def_rows(&n9[0]).unwrap();
    let n9b = def_rows(&n9[1]).unwrap();
    let (ra, rb) =
        (row(&n9a, "_ZNK3BoxIcE3getEv").unwrap(), row(&n9b, "_ZNK3BoxIcE3getEv").unwrap());
    assert_eq!((ra.flags, rb.flags), (IN | EXPLICIT_INST, IN | IMPLICIT_INST));
    assert_eq!(ra.hash, rb.hash);

    let st = "struct A { static constexpr int n = 3; };\n";
    std::fs::write(
        dir.join("n22a.cpp"),
        format!("{st}constexpr int A::n;\nconst int *pa() {{ return &A::n; }}\n"),
    )
    .unwrap();
    std::fs::write(dir.join("n22b.cpp"), format!("{st}const int *pb() {{ return &A::n; }}\n"))
        .unwrap();
    let strong = defs_of(
        &dir,
        "n22a.cpp",
        &std::fs::read_to_string(dir.join("n22a.cpp")).unwrap(),
        &["-std=c++14"],
    )
    .unwrap();
    let vague = defs_of(
        &dir,
        "n22b.cpp",
        &std::fs::read_to_string(dir.join("n22b.cpp")).unwrap(),
        &["-std=c++17"],
    )
    .unwrap();
    let (rs, rv) = (row(&strong, "_ZN1A1nE").unwrap(), row(&vague, "_ZN1A1nE").unwrap());
    assert_eq!((rs.flags, rv.flags), (0, IN), "N22: the strong row and the vague row");
    assert_eq!((rs.hash, rs.class), (rv.hash, rv.class));

    let tf = "template <class T> int tf(T) { return 1; }\n";
    let k13 = objects(
        &dir,
        &[
            (
                "k13a.cpp",
                &format!(
                    "{tf}template <> int tf<int>(int) {{ return 2; }}\nint a() {{ return tf(1); }}\n"
                ),
            ),
            ("k13b.cpp", &format!("{tf}int b() {{ return tf(1); }}\n")),
        ],
        &[],
    );
    let (spec_rows, inst_rows) = (def_rows(&k13[0]).unwrap(), def_rows(&k13[1]).unwrap());
    let (ra, rb) =
        (row(&spec_rows, "_Z2tfIiEiT_").unwrap(), row(&inst_rows, "_Z2tfIiEiT_").unwrap());
    assert_eq!((ra.flags, rb.flags), (EXPLICIT_SPEC, IN | IMPLICIT_INST));
    assert_ne!(ra.hash, rb.hash, "the bodies differ");

    // The oracle reports the K13 pair and nothing for N9.
    let oracle = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scripts/mason/odr_oracle.py");
    let py = |objs: &[&str]| {
        let mut c = std::process::Command::new("python3");
        c.arg("-I").arg(&oracle).arg("--names").args(objs).current_dir(&dir);
        c.output()
    };
    let Ok(k13_out) = py(&["k13a.cpp.o", "k13b.cpp.o"]) else {
        eprintln!("no python3: the oracle is not run");
        return;
    };
    let out = text(&k13_out.stdout);
    assert_eq!(k13_out.status.code(), Some(1), "{out}{}", text(&k13_out.stderr));
    assert!(
        out.contains("K13: _Z2tfIiEiT_: EXPLICIT_SPEC in k13a.cpp.o, IMPLICIT_INST in k13b.cpp.o"),
        "{out}"
    );
    assert!(out.contains("disagreeing keys 1 (non-PARTIAL 1), K13 pairs 1"), "{out}");
    let n9_out = py(&["n9a.cpp.o", "n9b.cpp.o"]).unwrap();
    let out = text(&n9_out.stdout);
    assert_eq!(n9_out.status.code(), Some(0), "{out}{}", text(&n9_out.stderr));
    assert!(out.contains("kind 2 rows 2, keys 1"), "{out}");
    assert!(out.contains("PARTIAL share 0.0%"), "{out}");
    assert!(out.contains("disagreeing keys 0 (non-PARTIAL 0), K13 pairs 0"), "{out}");
}

/// A definition's hash names the structors it calls by their
/// complete-object symbol whatever the unit emits: one unit constructs
/// the base alone (its complete-object constructor is emitted, at `-O0`
/// folded into the base-object one), the other only through the derived
/// class (found in fmt's tests, where `std::bad_alloc()` disagreed).
#[test]
fn structor_names_do_not_depend_on_emission() {
    let dir = scratch("defs-structor-names");
    let h = "struct E { E() {} ~E() {} };\nstruct X : E { X() {} ~X() {} };\n";
    // At `-O0` (the complete-object variant folded; at `-O2` both
    // structors are inlined away and the derived class has no rows).
    for o in ["-O0"] {
        let objs = objects(
            &dir,
            &[
                ("sa.cpp", &format!("{h}void a() {{ X x; }}\n")),
                ("sb.cpp", &format!("{h}void b() {{ X x; E e; }}\n")),
            ],
            &[o],
        );
        let (ra, rb) = (def_rows(&objs[0]).unwrap(), def_rows(&objs[1]).unwrap());
        for sym in ["_ZN1XC2Ev", "_ZN1XD2Ev"] {
            let (x, y) = (row(&ra, sym).unwrap(), row(&rb, sym).unwrap());
            assert_eq!((x.hash, x.flags), (y.hash, y.flags), "{o} {sym}");
        }
    }
}

/// The structor variants (ruling 1): every symbol a structor is emitted
/// under has a row, all with one hash and one set of flags; at `-O0`
/// a vague complete-object constructor is folded into the base-object
/// one (no C1 symbol), an explicit instantiation's C1 and D1 are aliases.
#[test]
fn structor_variants() {
    let d = scratch("defs-structors");
    let src = "template <class T> struct S { S() {} S(int) {} ~S() {} virtual void v() {} };\n\
               template struct S<int>;\n\
               struct P { P() {} virtual ~P() {} };\n\
               struct Q : P { Q() {} ~Q() {} };\n\
               inline void usep() { P p; Q q; P *h = new Q; delete h; }\n\
               void use() { S<long> s; usep(); }\n";
    let ir = ir_of(&d, "s.cpp", src, &["-O0"]);
    let rows = def_rows(&ir_bytes(meta_line(&ir).unwrap())).unwrap();
    let defs = defined(&ir);
    let structors: Vec<&String> = defs
        .iter()
        .filter(|s| s.contains('C') || s.contains('D'))
        .filter(|s| ["C1E", "C2E", "D0E", "D1E", "D2E"].iter().any(|v| s.contains(v)))
        .collect();
    let want = [
        "_ZN1SIiEC1Ev",
        "_ZN1SIiEC2Ev",
        "_ZN1SIiEC1Ei",
        "_ZN1SIiEC2Ei",
        "_ZN1SIiED1Ev",
        "_ZN1SIiED2Ev",
        "_ZN1SIlEC2Ev",
        "_ZN1SIlED2Ev",
        "_ZN1PC2Ev",
        "_ZN1PD0Ev",
        "_ZN1PD2Ev",
        "_ZN1QC2Ev",
        "_ZN1QD0Ev",
        "_ZN1QD2Ev",
    ];
    let mut got: Vec<&str> = structors.iter().map(|s| s.as_str()).collect();
    got.sort_unstable();
    got.dedup();
    let mut w = want.to_vec();
    w.sort_unstable();
    assert_eq!(got, w, "the structor symbols at -O0");
    for sym in want {
        assert!(row(&rows, sym).is_some(), "{sym}");
    }
    let same = |a: &str, b: &str| {
        let (x, y) = (row(&rows, a).unwrap(), row(&rows, b).unwrap());
        assert_eq!((x.hash, x.flags, x.line), (y.hash, y.flags, y.line), "{a} {b}");
    };
    same("_ZN1SIiEC1Ev", "_ZN1SIiEC2Ev");
    same("_ZN1SIiEC1Ei", "_ZN1SIiEC2Ei");
    same("_ZN1SIiED1Ev", "_ZN1SIiED2Ev");
    same("_ZN1PD0Ev", "_ZN1PD2Ev");
    same("_ZN1QD0Ev", "_ZN1QD2Ev");
    assert_eq!(row(&rows, "_ZN1SIiEC1Ev").unwrap().flags, IN | EXPLICIT_INST);
    assert_eq!(row(&rows, "_ZN1SIlEC2Ev").unwrap().flags, IN | IMPLICIT_INST);
    assert_ne!(row(&rows, "_ZN1SIiEC1Ev").unwrap().hash, row(&rows, "_ZN1SIiEC1Ei").unwrap().hash);
}

/// The decided forms: a function with an `asm` label is keyed by the
/// label; an `alias` or `ifunc` symbol gets no row (the aliasee does);
/// a coroutine's ramp has a row and its outlined parts (internal) none;
/// an `always_inline` inline function gets none (the always-inliner
/// deletes it at every `-O`). kiln rejects function multiversioning
/// (`target`, `target_clones`), so it has no rows to decide.
#[test]
fn decided_forms() {
    let d = scratch("defs-decide");
    // Header-free (the target is pinned): the coroutine library is
    // declared by hand, on line 1 so that the lines below stay put.
    let src = "namespace std { template <class R, class... A> struct coroutine_traits { using promise_type = typename R::promise_type; }; template <class P = void> struct coroutine_handle { void *p; static coroutine_handle from_address(void *a) noexcept { return {a}; } void *address() const noexcept { return p; } }; struct suspend_never { bool await_ready() const noexcept { return true; } template <class H> void await_suspend(H) const noexcept {} void await_resume() const noexcept {} }; }\n\
        struct task { struct promise_type { task get_return_object() { return {}; } std::suspend_never initial_suspend() noexcept { return {}; } std::suspend_never final_suspend() noexcept { return {}; } void return_void() {} void unhandled_exception() {} }; };\n\
        inline task co() { co_return; }\n\
        inline int lbl() asm(\"my_label\");\n\
        inline int lbl() { return 7; }\n\
        extern \"C\" inline int ali_t() { return 1; }\n\
        extern \"C\" int ali() __attribute__((alias(\"ali_t\")));\n\
        extern \"C\" int (*res())() { return ali_t; }\n\
        extern \"C\" int ifn() __attribute__((ifunc(\"res\")));\n\
        [[gnu::always_inline]] inline int ai() { return 3; }\n\
        struct AI { [[gnu::always_inline]] AI() {} [[gnu::always_inline]] ~AI() {} };\n\
        int use() { co(); AI x; return lbl() + ali() + ifn() + ai() + ali_t(); }\n";
    let ir = ir_of(&d, "d.cpp", src, &["-std=c++20", "--target=x86_64-linux-gnu"]);
    let rows = def_rows(&ir_bytes(meta_line(&ir).unwrap())).unwrap();
    assert_eq!(row(&rows, "my_label").map(|r| r.flags), Some(IN));
    assert_eq!(row(&rows, "_Z3lblv"), None);
    assert_eq!(row(&rows, "ali_t").map(|r| r.line), Some(6));
    for none in [
        "ali",
        "ifn",
        "res",
        "_Z2aiv",
        "_ZN2AIC2Ev",
        "_ZN2AID2Ev",
        "_Z2cov.resume",
        "_Z2cov.destroy",
        "_Z3usev",
    ] {
        assert_eq!(row(&rows, none), None, "{none}");
    }
    assert_eq!(
        row(&rows, "_Z2cov").map(|r| (r.flags, r.line)),
        Some((IN, 3)),
        "the ramp (a coroutine body, K-3d)"
    );
    std::fs::write(
        d.join("mv.cpp"),
        "__attribute__((target(\"default\"))) int mv() { return 1; }\n\
         __attribute__((target(\"avx2\"))) int mv() { return 2; }\n",
    )
    .unwrap();
    let r =
        run(&d, &["--target=x86_64-linux-gnu", "-fmason-metadata", "-c", "mv.cpp", "-o", "mv.o"]);
    assert!(
        text(&r.stderr).contains("function multiversioning with 'target' is not supported"),
        "{}",
        text(&r.stderr)
    );
}

/// Two library-heavy units per standard library (those of
/// `library_rows_agree_across_units` in `mason_meta.rs`) agree on every
/// shared key, in hash, class and flags, and have no K13 pair. Both
/// also call a common function, so that the sets overlap.
#[test]
fn library_rows_agree_across_units() {
    let d = scratch("defs-library");
    let hdr = "#include <vector>\n#include <string>\n#include <map>\n#include <optional>\n#include <variant>\n\
               static int common() { std::vector<std::string> v{\"a\"}; v.push_back(\"b\"); std::map<std::string, int> m; m[\"k\"] = 1; return (int)v.size() + m[\"k\"]; }\n";
    let a = format!(
        "{hdr}int a(std::map<std::string, std::vector<int>> &m) {{ std::optional<std::string> o; std::variant<int, std::string> v; return (int)m.size() + (int)o.has_value() + (int)v.index() + common(); }}\n"
    );
    let b = format!(
        "{hdr}std::string b(std::vector<std::string> &v) {{ std::map<std::string, std::vector<int>> m; return v.empty() ? std::string() : v[0] + std::to_string(m.size() + common()); }}\n"
    );
    for lib in ["-stdlib=libstdc++", "-stdlib=libc++"] {
        std::fs::write(d.join("a.cpp"), &a).unwrap();
        if !run(&d, &["-std=c++20", lib, "-fsyntax-only", "a.cpp"]).status.success() {
            eprintln!("skipping {lib}");
            continue;
        }
        let ra = defs_of(&d, "a.cpp", &a, &["-std=c++20", lib]).unwrap();
        let rb = defs_of(&d, "b.cpp", &b, &["-std=c++20", lib]).unwrap();
        assert!(ra.len() > 50 && rb.len() > 50, "{lib}: {} {}", ra.len(), rb.len());
        let mut shared = 0;
        for r in &ra {
            if let Some(s) = rb.iter().find(|s| s.key == r.key) {
                assert_eq!(r, s, "{lib}");
                shared += 1;
            }
        }
        assert!(shared > 20, "{lib}: {shared}");
        for r in ra.iter().chain(&rb) {
            assert!(r.flags & EXPLICIT_SPEC == 0 || r.flags & IMPLICIT_INST == 0, "{lib}: {r:?}");
        }
        assert!(ra.iter().filter(|r| r.flags & SYSTEM != 0).count() > ra.len() / 2, "{lib}");
    }
}

/// Must-equal across routes (mortar's counter-proposal b): a header
/// included textually and the same header through a PCH give
/// byte-equal kind 2 rows: same keys, hashes, flags and places.
#[test]
fn textual_and_pch_rows_equal() {
    let d = scratch("defs-pch");
    let (decls, uses) = KINDS.split_at(KINDS.find("int use()").unwrap());
    let extra = "int ext(int x = 2);\n\
                 struct M { int a = 4; int *p; M(int x) : p(new int(x)) {} virtual ~M() { delete p; } };\n\
                 inline int dm() { M m(ext()); int v[3] = {1}; return m.a + v[0] + *m.p; }\n\
                 template <class T> inline T lt(T t) { auto g = [t](auto u) { return [&] { return t + u; }(); }; struct L { T k; T get() { return k; } }; return g(1) + g(2L) + L{t}.get(); }\n\
                 inline int lam() { int y = 1; auto l = [=, &y](int x) mutable { y += x; return [y] { return y; }(); }; return l(1) + lt(1) + lt(2L); }\n\
                 template <bool B> struct EI {}; template <> struct EI<true> { using type = int; };\n\
                 template <class T, typename EI<(sizeof(T) > 1)>::type = 0> int sel(T) { return 1; }\n\
                 template <class T, typename EI<(sizeof(T) <= 1)>::type = 0> int sel(T) { return 2; }\n\
                 namespace { int hidden; }\n\
                 [[gnu::aligned(16)]] inline int k3e(int a, int b = 3) { [[assume(a > 0)]]; return sel(a) + sel('c') + b + hidden; }\n";
    std::fs::write(d.join("h.h"), format!("{decls}{extra}")).unwrap();
    ok(&d, &["-std=c++20", "-x", "c++-header", "h.h", "-o", "h.pch"]);
    let u = format!("{uses}int use2() {{ return dm() + lam() + k3e(1); }}\n");
    let with_pch = defs_of(&d, "u.cpp", &u, &["-std=c++20", "-include-pch", "h.pch"]).unwrap();
    let with_h = defs_of(&d, "u.cpp", &u, &["-std=c++20", "-include", "h.h"]).unwrap();
    assert_eq!(with_pch, with_h);
    assert!(with_h.len() > 25 && with_h.iter().all(|r| r.file.ends_with("h.h")), "{with_h:#?}");
    assert_ne!(row(&with_h, "_Z3k3eii").expect("k3e").flags & USES_INTERNAL, 0);
    for sym in ["_Z2dmv", "_ZN1MC2Ei", "_ZN1MD2Ev", "_Z3lamv", "_Z2ltIiET_S0_", "_Z3k3eii"] {
        let r = row(&with_h, sym).unwrap_or_else(|| panic!("no row for {sym}"));
        assert_eq!(r.flags & PARTIAL, 0, "{sym}");
    }
}

/// Without the flag nothing changes: the IR of a unit compiled without
/// it and with `-fno-mason-metadata` last is the same, and the flag adds
/// only the container (the global, its `@llvm.compiler.used` entry and
/// the `!exclude` node).
#[test]
fn flag_off_changes_nothing() {
    let d = scratch("defs-off");
    std::fs::write(d.join("k.cpp"), KINDS).unwrap();
    let base = ["-std=c++20", "-S", "-emit-llvm", "-o", "-", "k.cpp"];
    let off = ok(&d, &base);
    let mut a = vec!["-fmason-metadata", "-fno-mason-metadata"];
    a.extend(base);
    assert_eq!(ok(&d, &a), off);
    let mut a = vec!["-fmason-metadata"];
    a.extend(base);
    let on = ok(&d, &a);
    let code = |ir: &str| -> Vec<String> {
        ir.lines()
            .filter(|l| {
                !l.starts_with('!')
                    && !l.starts_with("@__mason_meta ")
                    && !l.starts_with("@llvm.compiler.used ")
            })
            .map(str::to_owned)
            .collect()
    };
    assert_eq!(code(&on), code(&off));
    assert!(!off.contains("mason"));
}

/// MT-38: the same unit compiled in two directories (with a prefix map)
/// gives the same section bytes, kind 2 included.
#[test]
fn same_bytes_from_two_directories() {
    let root = scratch("defs-determinism");
    let mut units = Vec::new();
    for dir in ["one", "two"] {
        let d = root.join(dir);
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("k.cpp"), KINDS).unwrap();
        let abs = d.canonicalize().unwrap().display().to_string();
        let map = format!("-ffile-prefix-map={abs}=/src");
        let src = format!("{abs}/k.cpp");
        ok(&d, &["-std=c++20", "-fmason-metadata", &map, "-c", &src, "-o", "k.o"]);
        let unit = container(&std::fs::read(d.join("k.o")).unwrap());
        let rows = def_rows(&unit).unwrap();
        assert!(rows.len() > 20 && rows.iter().all(|r| r.file == "/src/k.cpp"));
        units.push(unit);
    }
    assert_eq!(units[0], units[1]);
}

/// MT-41's gates on the heavy TU (`bench/frontend/format-like-g.cpp`) at
/// `default` with kind 2: the unit at most 125,000 B and 12 % of the
/// `-O0 -g` object.
#[test]
fn size_gate_with_kind_2() {
    let d = scratch("defs-size");
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../bench/frontend/format-like-g.cpp");
    let src = src.canonicalize().unwrap().display().to_string();
    ok(&d, &["-std=c++20", "-O0", "-g", "-fmason-metadata", "-c", &src, "-o", "f.o"]);
    let obj = std::fs::read(d.join("f.o")).unwrap();
    let unit = container(&obj);
    let rows = def_rows(&unit).unwrap();
    assert!(rows.len() > 100, "{}", rows.len());
    eprintln!(
        "format-like-g: unit {} B, object {} B, kind 2 {} rows",
        unit.len(),
        obj.len(),
        rows.len()
    );
    assert!(unit.len() <= 125_000, "{}", unit.len());
    assert!(unit.len() * 100 <= obj.len() * 12, "{} of {}", unit.len(), obj.len());
}

/// K11 (O-13): a lambda in an inline function whose body differs: the
/// enclosing function's row and the closure's `operator()` row differ;
/// the same source gives the same rows. The static invoker and the
/// conversion function get no row (O-14); a local class's members none
/// (their definitions are the enclosing function's); a generic lambda's
/// specializations each one, equal across units.
#[test]
fn closure_rows() {
    let dir = scratch("defs-closures");
    let src = |v: i32| {
        format!(
            "inline int f(int a) {{ auto l = [a](int x) {{ return x + a + {v}; }}; int (*p)(int) = [](int x) {{ return x; }}; return l(1) + p(2); }}\n\
             inline int gen() {{ auto g = [](auto x) {{ return x; }}; return g(1) + (int)g(2L); }}\n\
             inline int lc() {{ struct L {{ int get() const {{ return 7; }} }}; return L().get(); }}\n\
             int use() {{ return f(1) + gen() + lc(); }}\n"
        )
    };
    let ir = ir_of(&dir, "a.cpp", &src(1), &["-std=c++20"]);
    let a = def_rows(&ir_bytes(meta_line(&ir).unwrap())).unwrap();
    let b = defs_of(&dir, "b.cpp", &src(2), &["-std=c++20"]).unwrap();
    let a2 = defs_of(&dir, "c.cpp", &src(1), &["-std=c++20"]).unwrap();
    let key =
        |v: &[Def]| v.iter().map(|r| (r.key, r.hash, r.class, r.flags, r.line)).collect::<Vec<_>>();
    assert_eq!(key(&a), key(&a2));
    let defs = defined(&ir);
    let op = "_ZZ1fiENKUliE_clEi";
    assert!(defs.iter().any(|s| s == op), "{defs:?}");
    for sym in ["_Z1fi", op] {
        let (x, y) = (row(&a, sym).unwrap(), row(&b, sym).unwrap());
        assert_ne!(x.hash, y.hash, "{sym}");
        assert_eq!(x.flags & PARTIAL, 0, "{sym}");
    }
    // Unchanged closures and functions keep their rows.
    for sym in ["_ZZ1fiENKUliE0_clEi", "_Z3genv", "_Z2lcv"] {
        let strip = |r: Option<&Def>| r.map(|r| (r.key, r.hash, r.class, r.flags, r.line));
        assert_eq!(strip(row(&a, sym)), strip(row(&b, sym)), "{sym}");
        assert!(row(&a, sym).is_some(), "{sym}");
    }
    let specs: Vec<&String> =
        defs.iter().filter(|s| s.starts_with("_ZZ3genvENKUlT_E_clI")).collect();
    assert_eq!(specs.len(), 2, "{defs:?}");
    let (s0, s1) = (row(&a, specs[0]).unwrap(), row(&a, specs[1]).unwrap());
    assert_ne!(s0.hash, s1.hash);
    for sym in &defs {
        let thunk = sym.contains("__invoke") || sym.contains("cvPF");
        let local = sym.starts_with("_ZZ2lcvENK1L");
        if thunk || local {
            assert_eq!(row(&a, sym), None, "{sym}");
        }
    }
    assert!(defs.iter().any(|s| s.starts_with("_ZZ2lcvENK1L")), "{defs:?}");
}

/// The main risk of closure rows: numbering that depends on the order a
/// unit instantiates in. Two units instantiate the same templates holding
/// lambdas and the same generic lambda in opposite orders: every shared
/// key has the same row.
#[test]
fn instantiation_order_does_not_change_rows() {
    let d = scratch("defs-order");
    let hdr = "template <class T> inline T t(T v) { auto a = [v] { return v; }; auto b = [&](auto w) { return a() + w; }; return b(v) + [] { return T(1); }(); }\n\
               inline auto gl = [](auto x) { return [x](auto y) { return x + y; }(x); };\n";
    let one = format!("{hdr}int u1() {{ return t(1) + (int)t(2L) + gl(1) + (int)gl(2.0); }}\n");
    let two = format!("{hdr}int u2() {{ return (int)gl(2.0) + gl(1) + (int)t(2L) + t(1); }}\n");
    let ra = defs_of(&d, "one.cpp", &one, &["-std=c++20"]).unwrap();
    let rb = defs_of(&d, "two.cpp", &two, &["-std=c++20"]).unwrap();
    let mut shared = 0;
    for r in &ra {
        if let Some(s) = rb.iter().find(|s| s.key == r.key) {
            assert_eq!((r.hash, r.flags, r.line), (s.hash, s.flags, s.line), "{r:?}");
            shared += 1;
        }
    }
    assert!(shared >= 8, "{shared}: {ra:#?}");
}

/// Two units that include the same definitions, the second after a
/// declaration that spells a dependent name of their signatures
/// otherwise: every key both units have gives one row (K-3e, the callee
/// by entity), and `user` is not `PARTIAL`.
fn same_rows_after_another_spelling(dir: &str, hdr: &str, other: &str, user: &str) -> usize {
    let d = scratch(dir);
    let one = format!("{hdr}{user}int a() {{ return user(); }}\n");
    let two = format!("{hdr}{other}{user}int b() {{ return user(); }}\n");
    let ra = defs_of(&d, "one.cpp", &one, &["-std=c++20"]).unwrap();
    let rb = defs_of(&d, "two.cpp", &two, &["-std=c++20"]).unwrap();
    let u = row(&ra, "_Z4userv").expect("user's row");
    assert_eq!(u.flags & PARTIAL, 0, "{u:?}");
    let mut shared = 0;
    for r in &ra {
        if let Some(s) = rb.iter().find(|s| s.key == r.key) {
            assert_eq!((r.hash, r.flags), (s.hash, s.flags), "{r:?}\n{ra:#?}\n{rb:#?}");
            shared += 1;
        }
    }
    shared
}

const SIG_HDR: &str = "namespace ns {\n\
    template <bool B, class T = void> struct enable_if {};\n\
    template <class T> struct enable_if<true, T> { using type = T; };\n\
    template <bool B, class T = void> using enable_if_t = typename enable_if<B, T>::type;\n\
    template <class T> struct has { static constexpr bool value = sizeof(T) > 1; };\n\
    }\n";

/// fmt's shape (reduced from real code): overloads of a member template of
/// a class template that differ in an `enable_if` condition. Without the
/// callee by entity the unit that met `ns::has<T>::value` first mangles
/// the callee otherwise and `user`'s rows disagree.
#[test]
fn callee_by_entity_fmt_shape() {
    let hdr = format!(
        "{SIG_HDR}namespace ns {{ template <class X> struct W {{\n\
           template <class T, enable_if_t<has<T>::value, int> = 0> int on(T) {{ return 1; }}\n\
           template <class T, enable_if_t<!has<T>::value, int> = 0> int on(T) {{ return 2; }}\n\
         }}; }}\n"
    );
    let other = "template <class T, ns::enable_if_t<ns::has<T>::value, int> = 0> bool g(T) { return true; }\n";
    let user = "inline int user() { return ns::W<int>().on(1) + ns::W<int>().on('c'); }\n";
    assert!(same_rows_after_another_spelling("defs-k3e-fmt", &hdr, other, user) >= 2);
}

/// What names the other templated callees (K-3e): a member function of a
/// class template specialization by its symbol (no dependent part), a
/// variable template specialization by its symbol, a specialization of a
/// member template by entity (its template, scope `C<int>`, signature).
#[test]
fn callee_forms_across_units() {
    let other = "template <class T> int z(ns::enable_if_t<ns::has<T>::value, int>);\n\
                 template <class T, ns::enable_if_t<ns::has<T>::value, int> = 0> bool g(T) { return true; }\n";
    let member = format!(
        "{SIG_HDR}template <class X> struct C {{ int f(ns::enable_if_t<ns::has<X>::value, int> i) {{ return i; }} }};\n"
    );
    let user = "inline int user() { return C<int>().f(1); }\n";
    same_rows_after_another_spelling("defs-k3e-member", &member, other, user);
    let var = format!(
        "{SIG_HDR}template <class T> constexpr int v = ns::has<T>::value ? 1 : 2;\n\
         template <class T> inline int vv = v<T> + 1;\n"
    );
    let user = "inline int user() { return vv<int> + vv<char>; }\n";
    same_rows_after_another_spelling("defs-k3e-var", &var, other, user);
    let mt = format!(
        "{SIG_HDR}template <class X> struct C {{\n\
           template <class T, ns::enable_if_t<ns::has<T>::value, int> = 0> int m(T) {{ return 1; }}\n\
           template <class T, ns::enable_if_t<!ns::has<T>::value, int> = 0> int m(T) {{ return 2; }}\n\
         }};\n"
    );
    let user = "inline int user() { return C<int>().m(1) + C<long>().m('c'); }\n";
    assert!(same_rows_after_another_spelling("defs-k3e-mt", &mt, other, user) >= 2);
}

/// O-16 (K-3e): `USES_INTERNAL` on a row whose definition odr-uses an
/// internal entity or names a TU-local one, in both units (K12); not on
/// a constant read by value (N17), an unevaluated operand, or the
/// definition's own local types. A per-TU placeholder (an
/// unnamed-namespace type) sets it too; the hashes stay equal.
#[test]
fn uses_internal_across_units() {
    let d = scratch("defs-k3e-internal");
    let hdr = "namespace { int counter; struct Anon { int v; }; }\n\
               static int helper() { return 1; }\n\
               static const int k = 3;\n\
               inline int k12() { return ++counter; }\n\
               inline int stat() { return helper(); }\n\
               inline int anon_type() { return sizeof(Anon); }\n\
               inline int n17() { return k + 1; }\n\
               inline int unevaluated() { return sizeof(helper()) + sizeof(counter); }\n\
               inline int own() { struct L { int m = 2; }; return L().m; }\n\
               inline const int *addr() { return &k; }\n";
    let use_all =
        "{ return k12() + stat() + anon_type() + n17() + unevaluated() + own() + *addr(); }\n";
    let ra = defs_of(&d, "a.cpp", &format!("{hdr}int a() {use_all}"), &[]).unwrap();
    let rb = defs_of(&d, "b.cpp", &format!("{hdr}int b() {use_all}"), &[]).unwrap();
    for (sym, set) in [
        ("_Z3k12v", true),
        ("_Z4statv", true),
        ("_Z9anon_typev", true),
        ("_Z4addrv", true),
        ("_Z3n17v", false),
        ("_Z11unevaluatedv", false),
        ("_Z3ownv", false),
    ] {
        let (x, y) = (row(&ra, sym).expect(sym), row(&rb, sym).expect(sym));
        assert_eq!(x.flags & USES_INTERNAL != 0, set, "{sym}: {x:?}");
        assert_eq!((x.hash, x.flags), (y.hash, y.flags), "{sym}");
    }
}

/// O-10 applied to references (K-3e): an entity named in a definition
/// contributes no abi tag, visibility or
/// `exclude_from_explicit_instantiation`, its own or one inherited from
/// an enclosing class or inline namespace. Found in downstream code: a
/// library built with its hiding macro empty calls the untagged symbol
/// where users call the tagged one. Unit A tags, unit B does not; equal
/// tokens give equal rows, and a different callee overload still
/// differs.
#[test]
fn abi_tags_and_visibility_not_hashed() {
    let d = scratch("defs-k3e-abitag");
    let src = |tag: bool| {
        let (f, c, e) = if tag {
            (
                r#"__attribute__((__visibility__("hidden"), __exclude_from_explicit_instantiation__, __abi_tag__("t1")))"#,
                r#"__attribute__((__visibility__("hidden"), __abi_tag__("t1")))"#,
                r#"__attribute__((__visibility__("hidden")))"#,
            )
        } else {
            ("", "", "")
        };
        format!(
            "#define FTAG {f}\n#define CTAG {c}\n#define ETAG {e}\n\
             template <class T> struct S {{ FTAG int f() {{ return 1; }} }};\n\
             template <class T> struct CTAG S2 {{ int f() {{ return 2; }} }};\n\
             inline namespace v1 CTAG {{ template <class T> int g(T t) {{ return sizeof(t); }}\n\
               struct In {{ int m = 3; }}; inline In mk() {{ return In(); }} }}\n\
             namespace o {{ inline v1::In mk2() {{ return {{}}; }} }}\n\
             struct CTAG Tg {{}};\n\
             template <class T> int sig(T, Tg) {{ return 4; }}\n\
             namespace m {{ CTAG inline int gv = 5; }}\n\
             CTAG inline int g2 = 6;\n\
             struct CTAG Hold {{ enum ETAG E {{ e1 = 7 }}; }};\n\
             inline int h(long) {{ return 8; }}\n\
             {}\
             inline int u1() {{ return S<int>().f(); }}\n\
             inline int u2() {{ return S2<int>().f(); }}\n\
             inline int u3() {{ return g(1) + In().m + mk().m + o::mk2().m; }}\n\
             inline int u4() {{ return sig(1, Tg()); }}\n\
             inline int u5() {{ return m::gv + g2; }}\n\
             inline int u6() {{ Hold::E e = Hold::e1; return e + sizeof(Hold::E); }}\n\
             inline int u7() {{ return h(1); }}\n\
             int use() {{ return u1() + u2() + u3() + u4() + u5() + u6() + u7(); }}\n",
            if tag { "inline int h(int) { return 9; }\n" } else { "" }
        )
    };
    let ra = defs_of(&d, "a.cpp", &src(true), &[]).unwrap();
    let rb = defs_of(&d, "b.cpp", &src(false), &[]).unwrap();
    for (sym, equal) in [
        ("_Z2u1v", true),
        ("_Z2u2v", true),
        ("_Z2u3v", true),
        ("_Z2u4v", true),
        ("_Z2u5v", true),
        ("_Z2u6v", true),
        ("_Z2u7v", false),
    ] {
        let (x, y) = (row(&ra, sym).expect(sym), row(&rb, sym).expect(sym));
        assert_eq!(x.flags & PARTIAL, 0, "{sym}: {x:?}");
        assert_eq!(x.hash == y.hash, equal, "{sym}: {x:?} {y:?}");
    }
}

/// No byte of a row depends on the rest of the unit (WP-1636, reported
/// by mortar's maintainers): an attribute-free inline function hashes the
/// same with and without an unrelated listed attribute anywhere in the
/// unit, before or after it, or a header that declares one.
#[test]
fn attribute_free_rows_ignore_the_rest_of_the_unit() {
    let d = scratch("defs-1636");
    let one = |file: &str, before: &str, after: &str| {
        let src = format!("{before}\ninline void fr() {{}}\nvoid ua() {{ fr(); }}\n{after}\n");
        let rows = defs_of(&d, file, &src, &[]).unwrap();
        let r = row(&rows, "_Z2frv").expect("fr");
        (r.hash, r.flags)
    };
    let aligned = "struct alignas(8) S { long long a; };";
    let base = one("p0.cpp", "", "");
    for (i, (before, after)) in [
        (aligned, ""),
        ("", aligned),
        ("[[noreturn]] void die();", ""),
        ("#include <stddef.h>", ""),
    ]
    .into_iter()
    .enumerate()
    {
        assert_eq!(one(&format!("p{}.cpp", i + 1), before, after), base, "{before}{after}");
    }
}

/// The three `noreturn` spellings hash alike, and unlike none (WP-1637):
/// the GNU ones also mark the function type, which is not hashed. A
/// `noreturn` that arrives only through a typedef'd function type is a
/// miss (spec/abi.md "Misses"): pinned.
#[test]
fn noreturn_spellings_agree() {
    let d = scratch("defs-1637");
    let h = |file: &str, pre: &str, spell: &str| {
        let src = format!("{pre}{spell} inline void nr() {{ throw 1; }}\nvoid u() {{ nr(); }}\n");
        row(&defs_of(&d, file, &src, &[]).unwrap(), "_Z2nrv").expect("nr").hash
    };
    let a = h("n1.cpp", "", "[[noreturn]]");
    assert_eq!(a, h("n2.cpp", "", "[[gnu::noreturn]]"));
    assert_eq!(a, h("n3.cpp", "", "__attribute__((noreturn))"));
    let none = h("n4.cpp", "", "");
    assert_ne!(a, none);
    let td = "typedef void F() __attribute__((noreturn));\ninline F nr;\n";
    assert_eq!(h("n5.cpp", td, ""), none, "the typedef's bit is not seen");
}

/// A closure under a function type or a template argument is TU-local
/// only when the mangler would not make it visible (WP-1638, reported by
/// mortar's maintainers, the shape of libc++'s `std::visit` tables): a
/// lambda of an inline function or an inline variable is the same entity
/// in every unit; one of a `static` variable is not.
#[test]
fn visible_closures_are_not_internal() {
    let d = scratch("defs-1638");
    let src = "template <class V> struct Wrap { V v; };\n\
        template <class T, int N> struct Arr { T buf[N]; __attribute__((noinline)) const T& operator[](int i) const { return buf[i]; } };\n\
        template <class V> __attribute__((noinline)) int disp(Wrap<V>&& w) { return w.v(); }\n\
        template <class V> __attribute__((noinline)) int run(V v) { constexpr Arr<int (*)(Wrap<V>&&), 1> table = {{&disp<V>}}; return table[0](Wrap<V>{v}); }\n\
        __attribute__((noinline)) inline int g() { return run([] { return 1; }); }\n\
        int use() { return g(); }\n";
    let rows = defs_of(&d, "v.cpp", src, &[]).unwrap();
    for line in [2, 4] {
        let r = rows.iter().find(|r| r.line == line).expect("row");
        assert_eq!(r.flags & USES_INTERNAL, 0, "line {line}: {r:?}");
    }
    for (file, spec, set) in [("i.cpp", "inline", false), ("s.cpp", "static", true)] {
        let src = format!(
            "template <class V> struct Wrap {{ V v; }};\n{spec} auto lam = [] {{ return 1; }};\n\
             inline int k() {{ int (*p)(Wrap<decltype(lam)>&&) = nullptr; return p != nullptr; }}\n\
             int use() {{ return k(); }}\n"
        );
        let rows = defs_of(&d, file, &src, &[]).unwrap();
        let r = row(&rows, "_Z1kv").expect("k");
        assert_eq!(r.flags & USES_INTERNAL != 0, set, "{spec}: {r:?}");
    }
}
