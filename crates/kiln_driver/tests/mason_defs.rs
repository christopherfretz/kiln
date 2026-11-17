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
/// `module` 0, reserved 0, `PARTIAL` set, no flag kiln does not write,
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
    assert_eq!(value(&summary, "odr.scheme"), Some("kiln-odr-1.d0"));
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
            assert_ne!(d.flags & PARTIAL, 0, "{d:?}");
            assert_eq!(
                d.flags
                    & !(VAGUE | IMPLICIT_INST | EXPLICIT_INST | EXPLICIT_SPEC | PARTIAL | SYSTEM),
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

/// FNV-1a 128, written here from its definition.
fn fnv128(s: &str) -> u128 {
    let prime: u128 = (1 << 88) + 0x13b;
    s.bytes().fold(0x6c62_272e_07bb_0142_62b8_2175_6295_c58d, |h, b| {
        (h ^ u128::from(b)).wrapping_mul(prime)
    })
}

/// The hash of scheme `kiln-odr-1.d0`: the class, the declared type's
/// `<type>` encoding, and a fixed token for the body.
fn hash_of(class: u16, ty: &str) -> u128 {
    let tag = if class == FUNCTION { "function" } else { "variable" };
    fnv128(&format!("{tag}\0{ty}\0body not hashed"))
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
/// `VAGUE | PARTIAL`, the flags of a plain inline definition.
const IN: u16 = VAGUE | PARTIAL;

/// T-O-14: a row per definition kind, each symbol the object defines
/// for one, and nothing else.
#[test]
fn definition_rows_golden() {
    let d = scratch("defs-golden");
    let ir = ir_of(&d, "k.cpp", KINDS, &["-std=c++20"]);
    let rows = def_rows(&ir_bytes(meta_line(&ir).unwrap())).unwrap();
    #[rustfmt::skip]
    let golden: &[(&str, u16, u16, u32, &str)] = &[
        // Inline function and variable; a thread_local inline variable.
        ("_Z3inli", F, IN, 1, "FiiE"),
        ("ivar", V, IN, 2, "i"),
        ("tl", V, IN, 25, "i"),
        // Implicit instantiations: function, member function, static data
        // member, member template, variable template, an inline member.
        ("_Z2tfIiET_S0_", F, IN | IMPLICIT_INST, 3, "FiiE"),
        ("_ZNK3BoxIiE3getEv", F, IN | IMPLICIT_INST, 5, "KFivE"),
        ("_ZN3BoxIiE2smE", V, IN | IMPLICIT_INST, 4, "i"),
        ("_ZN3BoxIiE2mtIiEET_S2_", F, IN | IMPLICIT_INST, 4, "FiiE"),
        ("_Z2vtIiE", V, IN | IMPLICIT_INST, 7, "Ki"),
        ("_ZN3BoxIiE5inl_mEv", F, IN | IMPLICIT_INST, 4, "FvvE"),
        // Explicit instantiation definitions: a function and a class's
        // members (weak_odr: vague). The place is the template's.
        ("_Z2tfIcET_S0_", F, IN | EXPLICIT_INST, 3, "FccE"),
        ("_ZNK3BoxIcE3getEv", F, IN | EXPLICIT_INST, 5, "KFivE"),
        ("_ZN3BoxIcE2smE", V, IN | EXPLICIT_INST, 4, "i"),
        ("_ZN3BoxIcE5inl_mEv", F, IN | EXPLICIT_INST, 4, "FvvE"),
        // Explicit specializations: strong (K13) and inline; a member of
        // an explicitly specialized class (decided: EXPLICIT_SPEC, since
        // [temp.expl.spec]/5 defines it as the specialization's member,
        // never instantiated); an explicitly specialized member.
        ("_Z2tfIlET_S0_", F, PARTIAL | EXPLICIT_SPEC, 8, "FllE"),
        ("_Z2tfIsET_S0_", F, IN | EXPLICIT_SPEC, 9, "FssE"),
        ("_ZNK3BoxIdE3getEv", F, PARTIAL | EXPLICIT_SPEC, 13, "KFivE"),
        ("_ZNK3BoxIfE3getEv", F, PARTIAL | EXPLICIT_SPEC, 14, "KFivE"),
        // A static constexpr data member (inline since C++17).
        ("_ZN1A1nE", V, IN, 15, "Ki"),
        // Inline members and structors; implicitly defined structors
        // (the place is the class).
        ("_ZN1DC2Ev", F, IN, 16, "FvvE"),
        ("_ZN1DD2Ev", F, IN, 16, "FvvE"),
        ("_ZN1D1vEv", F, IN, 16, "FvvE"),
        ("_ZN3ImpC2Ev", F, IN, 17, "FvvE"),
        ("_ZN3ImpD2Ev", F, IN, 17, "FvvE"),
        // A closure's `operator()` (an inline variable's lambda) and a
        // generic lambda's specialization; the variables themselves (a
        // closure type is numbered per scope: hashed as per TU).
        ("_ZNK2fpMUliE_clEi", F, IN, 27, "KFiiE"),
        ("_ZNK2glMUlT_E_clIiEEDaS0_", F, IN | IMPLICIT_INST, 24, "KFiiE"),
        ("fp", V, IN, 27, "PFiiE"),
        ("gl", V, IN, 24, "?per-tu"),
        // A function with a static local (the local and its guard: none).
        ("_Z2slv", F, IN, 26, "FivE"),
    ];
    for &(sym, class, flags, line, ty) in golden {
        let r = row(&rows, sym).unwrap_or_else(|| panic!("no row for {sym}"));
        let want = Def {
            key: fnv(sym),
            hash: hash_of(class, ty),
            class,
            flags,
            file: "k.cpp".into(),
            line,
        };
        assert_eq!(r, &want, "{sym}");
    }
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
               int use() { return g() + h(1) + k(); }\n";
    let rows = defs_of(&d, "x.cpp", x15, &["-std=c++20"]).unwrap();
    assert_eq!(rows.iter().map(|r| r.line).collect::<Vec<_>>(), [3], "{rows:?}");
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
        assert_eq!(rows.map(|r| r.len()), Some(1), "{l}");
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
/// C++17's inline one), K13 (an explicit specialization against an
/// implicit instantiation): one key, equal hashes, the flags tell them
/// apart.
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
    assert_eq!((rs.flags, rv.flags), (PARTIAL, IN), "N22: the strong row and the vague row");
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
    assert_eq!((ra.flags, rb.flags), (PARTIAL | EXPLICIT_SPEC, IN | IMPLICIT_INST));
    assert_eq!(ra.hash, rb.hash, "the body is not hashed yet: only the flags tell");

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
    assert!(out.contains("disagreeing keys 0, K13 pairs 1"), "{out}");
    let n9_out = py(&["n9a.cpp.o", "n9b.cpp.o"]).unwrap();
    let out = text(&n9_out.stdout);
    assert_eq!(n9_out.status.code(), Some(0), "{out}{}", text(&n9_out.stderr));
    assert!(out.contains("kind 2 rows 2, keys 1"), "{out}");
    assert!(out.contains("PARTIAL share 100.0%"), "{out}");
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
    let src = "#include <coroutine>\n\
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
    assert_eq!(row(&rows, "my_label").map(|r| (r.flags, r.hash)), Some((IN, hash_of(F, "FivE"))));
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
    assert_eq!(row(&rows, "_Z2cov").map(|r| (r.flags, r.line)), Some((IN, 3)), "the ramp");
    // The coroutine's library code is in a system header.
    assert!(rows.iter().any(|r| r.flags & SYSTEM != 0 && r.file.ends_with("coroutine")));
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
