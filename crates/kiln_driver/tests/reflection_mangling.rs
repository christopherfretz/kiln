//! The mangling of reflection template arguments across translation
//! units (spec/reflection.md §8.3, WP-501, WP-879): the per-kind encoding
//! table in both forms (`-fkiln-reflection-mangling=kiln|gcc`), a two-TU
//! program whose reflection-keyed entities link to one definition each in
//! either form, and link compatibility with g++-16 (skipped without
//! `KILN_GCC`), including signatures with a dependent reflected
//! template-id (WP-1250). Reflections
//! are kiln and GCC only (clang has no `^^`), so these read the IR and the
//! objects directly instead of comparing with an oracle.
#![forbid(unsafe_code)]

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

fn kilnpp() -> PathBuf {
    Path::new(env!("CARGO_BIN_EXE_kiln")).with_file_name("kiln++")
}

/// `tests/support`, for the `std::meta` declarations `kiln_meta.h` makes.
fn support() -> String {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/support").display().to_string()
}

fn scratch(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("kiln-refl-mangle-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn have(tool: &str) -> bool {
    Command::new(tool).arg("--version").stdout(Stdio::null()).stderr(Stdio::null()).status().is_ok()
}

fn run(cmd: &mut Command) -> Output {
    let o = cmd.output().unwrap();
    assert!(o.status.success(), "{cmd:?}: {}", String::from_utf8_lossy(&o.stderr));
    o
}

fn kiln(dir: &Path, args: &[&str]) -> Output {
    let i = format!("-I{}", support());
    run(Command::new(kilnpp()).args(["-std=c++26", "-freflection", &i]).args(args).current_dir(dir))
}

/// `(name, nm letter)` for every symbol `nm` lists in `obj`.
fn nm(dir: &Path, obj: &str) -> Vec<(String, String)> {
    let o = run(Command::new("nm").arg(obj).current_dir(dir));
    String::from_utf8_lossy(&o.stdout)
        .lines()
        .filter_map(|l| {
            let mut w = l.split_whitespace().rev();
            let name = w.next()?;
            Some((name.to_owned(), w.next()?.to_owned()))
        })
        .collect()
}

/// Every kind of reflection GCC 16.1 spells ([meta.reflection]), as
/// `(kind letters, operand, kiln form, gcc form)`: the template argument
/// of `f<operand>` between `_Z1fIL` and `EEiv`. The gcc column is
/// byte-identical to g++-16 (`-std=c++26 -freflection`, `<meta>`).
const KINDS: &[(&str, &str, &str, &str)] = &[
    ("nu", "info{}", "u6__infonu", "Dmnu"),
    ("ty", "^^int", "u6__infotyi", "Dmtyi"),
    ("ty", "^^const int*", "u6__infotyPKi", "DmtyPKi"),
    ("ty", "^^TT<int>", "u6__infoty2TTIiE", "Dmty2TTIiE"),
    ("ta", "^^I", "u6__infota1I_i", "Dmta1I_i"),
    ("ta", "^^ns::A", "u6__infota2ns1A_NS1_1SE", "Dmta2ns1A_NS0_1SE"),
    ("ns", "^^ns", "u6__infons2ns", "Dmns2ns"),
    ("gs", "^^::", "u6__infogs", "Dmgs"),
    ("na", "^^na", "u6__infona2na", "Dmna2na"),
    ("vr", "^^var", "u6__infovr3var", "Dmvr3var"),
    ("vr", "^^S::sm", "u6__infovrN1S2smE", "DmvrN1S2smE"),
    ("fn", "^^g", "u6__infofn1gv", "Dmfn1gv"),
    ("fn", "^^S::mf", "u6__infofnN1S2mfEv", "DmfnN1S2mfEv"),
    ("fnC4", "ctor_of(^^ns::S)", "u6__infofnN2ns1SC4Ev", "DmfnN2ns1SC4Ev"),
    ("fnD4", "dtor_of(^^ns::S)", "u6__infofnN2ns1SD4Ev", "DmfnN2ns1SD4Ev"),
    ("dm", "^^S::dm", "u6__infodm1S2dm", "Dmdm1S2dm"),
    ("dm", "^^ns::S::u1", "u6__infodm2ns1S2u1", "Dmdm2ns1S2u1"),
    (
        "dm",
        "nonstatic_data_members_of(^^ns::S, access_context::unchecked())[1]",
        "u6__infodm2ns1S1b",
        "Dmdm2ns1S1b",
    ),
    ("en", "^^S::e1", "u6__infoen1S1E2e1", "Dmen1S1E2e1"),
    ("en", "^^EC::b", "u6__infoen2EC1b", "Dmen2EC1b"),
    ("ct", "^^TT", "u6__infoct2TT", "Dmct2TT"),
    ("ft", "^^tf", "u6__infoft2tf", "Dmft2tf"),
    ("vt", "^^tv", "u6__infovt2tv", "Dmvt2tv"),
    ("at", "^^TA", "u6__infoat2TA", "Dmat2TA"),
    ("co", "^^C", "u6__infoco1C", "Dmco1C"),
    ("sb", "^^sb2", "u6__infosb3sb2", "Dmsb3sb2"),
    ("pa", "parameters_of(^^pfn)[1]", "u6__infopa0_3pfnid", "Dmpa0_3pfnid"),
    ("ba", "bases_of(^^D2, access_context::unchecked())[1]", "u6__infoba0_2D2", "Dmba0_2D2"),
    ("vl", "reflect_constant(42)", "u6__infovlLi42E", "DmvlLi42E"),
    ("vl", "reflect_constant('c')", "u6__infovlLc99E", "DmvlLc99E"),
    ("vl", "reflect_constant(true)", "u6__infovlLb1E", "DmvlLb1E"),
    ("vl", "reflect_constant(EC::a)", "u6__infovlL2EC0E", "DmvlL2EC0E"),
    ("vl", "reflect_constant(nullptr)", "u6__infovlLDnE", "DmvlLDnE"),
    (
        "vl",
        "reflect_constant(Lit{3})",
        "u6__infoobL_Z17_ZTAXtl3LitLi3EEEE",
        "DmobL_Z17_ZTAXtl3LitLi3EEEE",
    ),
    ("vl", "reflect_constant(&arr[1])", "u6__infovladixL_Z3arrELl1E", "DmvladixL_Z3arrELl1E"),
    ("vl", "reflect_constant(^^int)", "u6__infovlLS0_tyiE", "DmvlLDmtyiE"),
    ("ob", "reflect_object(var)", "u6__infoobL_Z3varE", "DmobL_Z3varE"),
    ("ob", "reflect_object(arr[1])", "u6__infoobixL_Z3arrELl1E", "DmobixL_Z3arrELl1E"),
    ("ds", "data_member_spec(^^int, {.name = \"x\"})", "u6__infodsi_1x___", "Dmdsi_1x___"),
    ("ds", "data_member_spec(^^int, {.bit_width = 3})", "u6__infodsi___3_", "Dmdsi___3_"),
    ("an", "annotations_of(^^Ann)[0]", "u6__infoan_", "Dman_"),
];

const KINDS_DECLS: &str = r"using namespace std::meta;
template <info R> int f() { return 0; }
struct S { int dm; static int sm; int mf(); enum E { e1 }; };
int S::sm = 0;
enum class EC { a, b };
using I = int;
int var;
int g() { return 0; }
namespace ns {
struct S { int dm; int : 3; int b : 2; union { int u1; }; S(); ~S(); };
using A = S;
}
namespace na = ns;
template <class T> struct TT {};
template <class T> int tf(T);
template <class T> int tv = 0;
template <class T> concept C = true;
template <class T> using TA = T*;
struct B1 {}; struct B2 {}; struct D2 : B1, B2 {};
int pfn(int p0, double p1);
struct [[=1]] Ann {};
struct Lit { int a; };
int arr[2] = {1, 2};
auto [sb1, sb2] = arr;
consteval info ctor_of(info c) {
  for (info m : members_of(c, access_context::unchecked())) if (is_constructor(m)) return m;
  return {};
}
consteval info dtor_of(info c) {
  for (info m : members_of(c, access_context::unchecked())) if (is_destructor(m)) return m;
  return {};
}
";

/// The `f<…>` symbols `@table` points at, in order.
fn table(ll: &str) -> Vec<String> {
    let t = ll.lines().find(|l| l.starts_with("@table = ")).expect("@table");
    t.split('@')
        .skip(2)
        .map(|s| s.chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '_').collect())
        .collect()
}

/// [temp.type]/2, [basic.link] P2996R13: every reflection kind has one
/// spelling per form, distinct reflections are distinct symbols, and
/// every specialization keyed on a reflection of an entity with linkage
/// is a `linkonce_odr` definition (one per program); an annotation's is
/// internal.
#[test]
fn per_kind_table() {
    let d = scratch("kinds");
    let mut src = String::from("#include \"kiln_meta_queries.h\"\n");
    src.push_str(KINDS_DECLS);
    src.push_str("int (*table[])() = {\n");
    for (_, e, _, _) in KINDS {
        let _ = writeln!(src, "  &f<{e}>,");
    }
    src.push_str("};\n");
    std::fs::write(d.join("k.cpp"), &src).unwrap();
    // No flag is the gcc form (the default, WP-909).
    for (mode, col) in [("kiln", 2), ("gcc", 3), ("default", 3)] {
        let flag = format!("-fkiln-reflection-mangling={mode}");
        let mut args = vec!["-S", "-emit-llvm", "-o", "k.ll", "k.cpp"];
        if mode != "default" {
            args.insert(0, &flag);
        }
        kiln(&d, &args);
        let ll = std::fs::read_to_string(d.join("k.ll")).unwrap();
        let got = table(&ll);
        assert_eq!(got.len(), KINDS.len(), "{mode}");
        for (row, g) in KINDS.iter().zip(&got) {
            let want = if col == 2 { row.2 } else { row.3 };
            assert_eq!(g, &format!("_Z1fIL{want}EEiv"), "{mode}: {} f<{}>", row.0, row.1);
            // An annotation's index is TU-local, so its specializations
            // are internal (spec/reflection.md §8.3 "Linkage").
            let link = if row.0 == "an" { "internal" } else { "linkonce_odr" };
            assert!(
                ll.lines().any(|l| l.starts_with(&format!("define {link} "))
                    && l.contains(&format!(" @{g}() "))),
                "{mode}: {g} is not {link}"
            );
        }
        assert_eq!(got.iter().collect::<BTreeSet<_>>().len(), got.len(), "{mode}: distinct");
    }
    let _ = std::fs::remove_dir_all(&d);
}

const COMMON: &str = r#"#include <typeinfo>
#include "kiln_meta.h"
using namespace std::meta;
struct S { int dm; };
using Alias = int;
namespace ns { inline int nv = 0; }
inline int func() { return 0; }
template <info R> struct T { static int count; };
template <info R> int T<R>::count = 0;
template <info R> int* f() { static int n; return &n; }
template <info R> int cnt() { return ++T<R>::count; }
struct Seen { void* p[7]; const std::type_info* ti; int c; };
Seen seen_a();
Seen seen_b();
#define SEEN(c) Seen{{&T<^^int>::count, f<^^int>(), f<^^Alias>(), f<^^ns::nv>(), \
    f<^^S::dm>(), f<^^func>(), (void*)&cnt<^^S>}, &typeid(T<^^Alias>), c}
"#;

/// [basic.def.odr]/15, [temp.type]/2 P2996R13: two translation units
/// that name the same reflection-keyed specializations get one entity
/// each (equal addresses, one `typeid`, one shared static counter), and
/// the objects carry them as weak (`W`/`V`), never TU-local. Run in the
/// default (gcc, `LDm…`) form and in the kiln (`Lu6__info…`) form.
#[test]
fn two_translation_units_link() {
    if !(have("clang++-19") && have("nm")) {
        eprintln!("skipping: clang++-19 / nm not found");
        return;
    }
    for (flag, info) in [(None, "Dm"), (Some("-fkiln-reflection-mangling=kiln"), "u6__info")] {
        two_tu(flag, info);
    }
}

fn two_tu(flag: Option<&str>, info: &str) {
    let d = scratch(&format!("two-tu-{info}"));
    std::fs::write(d.join("common.h"), COMMON).unwrap();
    std::fs::write(
        d.join("a.cpp"),
        r#"#include "common.h"
#include <cstdio>
Seen seen_a() { return SEEN(cnt<^^S>()); }
int main() {
  Seen a = seen_a(), b = seen_b();
  int same = 0;
  for (int i = 0; i < 7; ++i) same += a.p[i] == b.p[i];
  std::printf("%d %d %d %d\n", same, a.ti == b.ti, *a.ti == *b.ti, a.c + b.c);
}
"#,
    )
    .unwrap();
    std::fs::write(
        d.join("b.cpp"),
        "#include \"common.h\"\nSeen seen_b() { return SEEN(cnt<^^S>()); }\n",
    )
    .unwrap();
    for tu in ["a.cpp", "b.cpp"] {
        let mut args: Vec<&str> = flag.into_iter().collect();
        args.extend(["-c", tu]);
        kiln(&d, &args);
    }
    run(Command::new(kilnpp()).args(["a.o", "b.o", "-o", "prog"]).current_dir(&d));
    let o = run(&mut Command::new(d.join("prog")));
    // Seven equal addresses, one type_info, and the counter reached 1
    // then 2 (1 + 2 = 3).
    assert_eq!(String::from_utf8_lossy(&o.stdout), "7 1 1 3\n");
    for obj in ["a.o", "b.o"] {
        let lit = format!("IL{info}");
        let syms: Vec<_> = nm(&d, obj).into_iter().filter(|(n, _)| n.contains(&lit)).collect();
        let names: BTreeSet<_> = syms.iter().map(|(n, _)| n.as_str()).collect();
        for want in [
            "_ZN1TILu6__infotyiEE5countE",
            "_Z1fILu6__infotyiEEPiv",
            "_Z1fILu6__infota5Alias_iEEPiv",
            "_Z1fILu6__infovrN2ns2nvEEEPiv",
            "_Z1fILu6__infodm1S2dmEEPiv",
            "_Z1fILu6__infofn4funcvEEPiv",
            "_Z3cntILu6__infoty1SEEiv",
            "_ZTI1TILu6__infota5Alias_iEE",
        ] {
            let want = want.replace("u6__info", info);
            assert!(names.contains(want.as_str()), "{obj}: {want} missing from {syms:?}");
        }
        for (n, l) in &syms {
            assert!(matches!(l.as_str(), "W" | "V" | "U"), "{obj}: {n} is {l}, not weak");
        }
    }
    let _ = std::fs::remove_dir_all(&d);
}

/// The specializations both compilers emit for one source, named without
/// the library (`decltype(^^int)` is the info type).
const COMPAT: &str = r"using info = decltype(^^int);
template <info R> int f() { return 0; }
template <info A, info B> int two() { return 0; }
template <info R> struct K { static int c; };
template <info R> int K<R>::c = 0;
template <class T> int dep(K<^^T>* = nullptr) { return 0; }
struct S { int dm; static int sm; int mf(); enum E { e1 }; S(); };
enum class EC { a, b };
using I = int;
int var;
int g() { return 0; }
namespace ns { int x; namespace inner { struct Q {}; } }
namespace na = ns;
template <class T> struct TT {};
template <class T> int tf(T);
template <class T> int tv = 0;
template <class T> concept C = true;
template <class T> using TA = T*;
int use() {
  return f<^^int>() + f<^^I>() + f<^^var>() + f<^^g>() + f<^^S::dm>() + f<^^S::sm>() + f<^^S::mf>()
       + f<^^S::e1>() + f<^^EC::b>() + f<^^ns>() + f<^^::>() + f<^^na>() + f<^^ns::inner::Q>()
       + f<^^TT>() + f<^^tf>() + f<^^tv>() + f<^^C>() + f<^^TA>() + f<info{}>() + f<^^TT<int>>()
       + f<^^ns::x>() + f<^^const int*>() + two<^^S, ^^S>() + two<^^S::dm, ^^S>() + K<^^int>::c
       + dep<int>() + dep<S>();
}
";

/// GCC link compatibility (spec/reflection.md §8.3): in the default
/// form (`-fkiln-reflection-mangling=gcc`, WP-909), kiln names every
/// reflection-keyed symbol exactly as g++-16 does. Skipped unless `KILN_GCC` names g++-16.
#[test]
fn gcc_link_compat() {
    let Some(gcc) = std::env::var_os("KILN_GCC").filter(|g| !g.is_empty()) else {
        eprintln!("skipping: KILN_GCC is not set");
        return;
    };
    if !(have("clang++-19") && have("nm")) {
        eprintln!("skipping: clang++-19 / nm not found");
        return;
    }
    let d = scratch("gcc");
    std::fs::write(d.join("t.cpp"), COMPAT).unwrap();
    kiln(&d, &["-c", "t.cpp", "-o", "k.o"]);
    run(Command::new(&gcc)
        .args(["-std=c++26", "-freflection", "-c", "t.cpp", "-o", "g.o"])
        .current_dir(&d));
    let names = |obj| -> BTreeSet<String> {
        nm(&d, obj).into_iter().map(|(n, _)| n).filter(|n| n.contains("Dm")).collect()
    };
    let (k, g) = (names("k.o"), names("g.o"));
    assert!(g.len() >= 25, "{g:?}");
    assert_eq!(k, g);
    let _ = std::fs::remove_dir_all(&d);
}

/// Signatures holding a dependent reflect-expression of an alias or a
/// variable template-id (spec/abi.md §8.14, WP-1250), in the three
/// mangled contexts; `DEF` defines them, `CALL` only declares them.
const TEMPLATE_ID_SIGS: &str = r"using info = decltype(^^int);
template <info R> struct K {};
template <class T> struct TT {};
template <class T> using AT = TT<T*>;
template <class T> constexpr int vt = 0;
#ifdef DEF
#define BODY(v) { return v; }
#define DBODY { return {}; }
#else
#define BODY(v) ;
#define DBODY ;
#endif
template <class T> auto d_at(T) -> decltype(K<^^AT<T>>{}) DBODY
template <class T> auto d_vt(T) -> decltype(K<^^vt<T>>{}) DBODY
template <class T> int p_at(T, K<^^AT<T>>) BODY(1)
template <class T> int p_vt(T, K<^^vt<T>>) BODY(2)
template <class T> requires(^^AT<T> != ^^int) int q_at(T) BODY(4)
template <class T> requires(^^vt<T> != ^^int) int q_vt(T) BODY(8)
template <class T> int p_ta(T, K<^^AT<T>>) BODY(16)
template <class T> int p_ta(T, K<^^TT<T*>>) BODY(32)
#ifdef DEF
template K<^^AT<int>> d_at(int);
template K<^^vt<int>> d_vt(int);
template int p_at(int, K<^^AT<int>>);
template int p_vt(int, K<^^vt<int>>);
template int q_at(int);
template int q_vt(int);
template int p_ta(int, K<^^AT<int>>);
template int p_ta(int, K<^^TT<int*>>);
#else
int main() {
  d_at(1);
  d_vt(1);
  return p_at(1, K<^^AT<int>>{}) + p_vt(1, K<^^vt<int>>{}) + q_at(1) + q_vt(1)
       + p_ta(1, K<^^AT<int>>{}) + p_ta(1, K<^^TT<int*>>{});
}
#endif
";

/// A kiln object links against a g++-16 one through signatures with a
/// dependent reflected alias or variable template-id (spec/abi.md §8.14,
/// [temp.over.link]/5, WP-1250): kiln defines and explicitly instantiates
/// them under the default `-fkiln-reflection-mangling=gcc`, g++-16
/// compiles the caller, and the program runs and returns the sum of
/// every call; both compilers' definitions also carry the same symbols.
/// Skipped unless `KILN_GCC` names g++-16.
#[test]
fn gcc_dependent_template_id_link() {
    let Some(gcc) = std::env::var_os("KILN_GCC").filter(|g| !g.is_empty()) else {
        eprintln!("skipping: KILN_GCC is not set");
        return;
    };
    if !have("nm") {
        eprintln!("skipping: nm not found");
        return;
    }
    let d = scratch("tid");
    std::fs::write(d.join("t.cpp"), TEMPLATE_ID_SIGS).unwrap();
    kiln(&d, &["-DDEF", "-c", "t.cpp", "-o", "k.o"]);
    let gcc_cmd = || {
        let mut c = Command::new(&gcc);
        c.args(["-std=c++26", "-freflection"]).current_dir(&d);
        c
    };
    run(gcc_cmd().args(["-DDEF", "-c", "t.cpp", "-o", "g.o"]));
    run(gcc_cmd().args(["-c", "t.cpp", "-o", "call.o"]));
    let defined = |obj| -> BTreeSet<String> {
        nm(&d, obj)
            .into_iter()
            .filter(|(n, l)| l == "W" && n.contains("Dm") && !n.ends_with("IiEiT_"))
            .map(|(n, _)| n)
            .collect()
    };
    let (k, g) = (defined("k.o"), defined("g.o"));
    assert_eq!(k.len(), 8, "{k:?}");
    assert_eq!(k, g);
    run(gcc_cmd().args(["call.o", "k.o", "-o", "a.out"]));
    let st = Command::new(d.join("a.out")).status().unwrap();
    assert_eq!(st.code(), Some(63));
    let _ = std::fs::remove_dir_all(&d);
}
