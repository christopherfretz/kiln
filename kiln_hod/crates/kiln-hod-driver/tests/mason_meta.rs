//! WP-1561: `-fmason-metadata` through hod (spec/codegen.md §5.5,
//! "Link-time metadata (`.mason.meta`)"). The `.mason.meta` section an
//! object gets from hod, in process and as the external executable, has
//! the LLVM route's type, flags, entry size, link, info and alignment, no
//! relocation section, and kiln's unit byte for byte first; after it, and
//! only there, hod appends its own unit (hod's spec/codegen.md §9.7.5:
//! producer 2, 88 bytes, one kind 1 row `reloc-model`), the one
//! difference from `llc` hod's spec permits. Read with the wrapper's
//! LLVM's `llvm-readelf -S` and `-x .mason.meta`.
//!
//! The inputs are WP-1557's (`crates/kiln_driver/tests/mason_meta.rs`)
//! plus one unit with every record kind (1 to 5, at `all`), which the test
//! checks by decoding the unit's block headers. The external route needs
//! `KILN_HOD_EXTERNAL` (hod's executable; `cargo xtask kiln-hod test` sets
//! it) and is skipped without it.
#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};
use std::process::Command;

const CXX: &str = "inline int h() { return 2; }\nint f() { return h(); }\n";
const C: &str = "int g(void) { return 1; }\n";

/// WP-1557's unit with one entity of each kind 5 category.
const DECLS_CXX: &str = r#"
int defined_fn(int a) noexcept { return a; }
int declared_fn(long);
struct S { int m(int) const; static int sm; S(); ~S(); };
int S::sm = 1;
extern int arr[];
int arr2[4];
extern "C" int cvar;
extern "C" int cfun(const char *, ...);
static int internal_fn() { return 1; }
namespace { int anon_v; }
int renamed() asm("renamed_label");
__attribute__((weak)) int weak_def() { return 0; }
inline int inl() { return 3; }
template <class T> T tf(T t) { return t; }
struct Inc;
extern Inc inc_v;
int use() {
  extern int blk();
  S s;
  return defined_fn(1) + declared_fn(2) + s.m(1) + S::sm + arr[0] + arr2[0] + cvar + cfun("")
       + internal_fn() + anon_v + renamed() + weak_def() + inl() + tf(1) + blk()
       + (int)(long)&inc_v;
}
"#;

/// WP-1557's C unit.
const DECLS_C: &str = r"
enum E { EA, EB };
typedef long L;
struct node;
int c_def(int a, const char *s) { return a + s[0]; }
int c_ref(enum E, const L, struct node *, double[static 3]);
extern int c_arr[];
int c_var = 1;
_Atomic long c_at;
int noproto();
int knr(a) int a; { return a; }
void abort(void);
unsigned long strlen(const char *);
double c_d[3];
int c_use(void) {
  return c_ref(EA, 0, 0, c_d) + c_arr[0] + (int)c_at + noproto() + knr(1)
       + (int)strlen((const char *)c_arr) + (c_var ? 0 : (abort(), 0));
}
";

/// Every record kind at `all`: the summary (1), definition hashes of
/// vague-linkage definitions (2), class layouts (3) and their details (4),
/// declarations (5).
const EVERY_KIND: &str = r"
struct Base { virtual ~Base(); virtual int f() const { return 1; } int b = 0; };
struct Derived : Base { int f() const override { return d; } long d = 2; };
template <class T> struct Box { T v; T get() const { return v; } };
inline int twice(int x) { return 2 * x; }
template <class T> T sum(T a, T b) { return a + b; }
extern int counter;
int declared(Box<long> *);
Base::~Base() = default;
int use(const Derived &d, Box<int> b) {
  Derived local;
  return d.f() + b.get() + twice(counter) + sum(1, 2) + declared(nullptr) + local.f();
}
";

/// The wrapper, as `kiln++` (C files go through `kiln`).
fn wrapper(c: bool) -> PathBuf {
    let kiln = PathBuf::from(env!("CARGO_BIN_EXE_kiln"));
    if c { kiln } else { kiln.with_file_name("kiln++") }
}

/// The wrapper's LLVM directory, from `--version`; `None` skips.
fn llvm_dir() -> Option<PathBuf> {
    let out = Command::new(wrapper(true)).arg("--version").output().expect("run the wrapper");
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    let dir = text
        .lines()
        .filter_map(|l| l.strip_prefix("LLVM backend: "))
        .find_map(|l| Some(PathBuf::from(l.split_once(" (")?.1.strip_suffix(')')?)));
    if dir.is_none() {
        eprintln!("skipping: the wrapper found no LLVM installation");
    }
    dir
}

/// The three routes.
#[derive(Clone, Copy, Debug)]
enum Route {
    Llvm,
    InProcess,
    External,
}

/// Compiles `src` to `<dir>/<obj>` by `route`; the statistics line is
/// checked to name the route.
fn compile(dir: &Path, src: &str, args: &[&str], route: Route, hod: Option<&Path>, obj: &str) {
    let log = dir.join(format!("{obj}.jsonl"));
    let _ = std::fs::remove_file(&log);
    let mut cmd = Command::new(wrapper(Path::new(src).extension().is_some_and(|e| e == "c")));
    cmd.current_dir(dir).args(args).args(["-c", src, "-o", obj]);
    cmd.env("KILN_STATS_LOG", &log).env_remove("KILN_BACKEND").env_remove("KILN_HOD");
    match route {
        Route::Llvm => {}
        Route::InProcess => {
            cmd.arg("-fkiln-backend=hod").env("KILN_HOD_STRICT", "1");
        }
        Route::External => {
            cmd.arg("-fkiln-backend=hod")
                .env("KILN_HOD_STRICT", "1")
                .env("KILN_HOD_IN_PROCESS", "0")
                .env("KILN_HOD", hod.expect("the external route needs hod"));
        }
    }
    let out = cmd.output().expect("run the wrapper");
    assert!(
        out.status.success(),
        "{src} {args:?} {route:?}: {}\n{}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );
    let line = std::fs::read_to_string(&log).unwrap();
    let want = match route {
        Route::Llvm => "\"backend\":\"llvm\"",
        Route::InProcess | Route::External => "\"backend\":\"hod\"",
    };
    assert!(line.contains(want), "{src} {route:?}: {line}");
    assert_eq!(line.contains("\"in_process\":true"), matches!(route, Route::InProcess), "{line}");
}

/// `llvm-readelf -S --wide`'s `.mason.meta` line without its address and
/// offset (Type, Size, ES, Flg, Lk, Inf, Al), whether a
/// `.rela.mason.meta` exists, and `-x .mason.meta`'s bytes.
fn section(readelf: &Path, obj: &Path) -> (Vec<String>, bool, Vec<u8>) {
    let out = Command::new(readelf).args(["-S", "--wide"]).arg(obj).output().unwrap();
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    let line = text
        .lines()
        .find(|l| l.contains(" .mason.meta "))
        .unwrap_or_else(|| panic!("{}: no .mason.meta\n{text}", obj.display()));
    let fields: Vec<&str> =
        line.split_once(" .mason.meta ").unwrap().1.split_whitespace().collect();
    // Type Address Off Size ES Flg Lk Inf Al
    let shape: Vec<String> = [0, 3, 4, 5, 6, 7, 8]
        .iter()
        .map(|&i| fields.get(i).copied().unwrap_or("").to_owned())
        .collect();
    let rela = text.contains(".rela.mason.meta") || text.contains(".rel.mason.meta");
    let out = Command::new(readelf).args(["-x", ".mason.meta"]).arg(obj).output().unwrap();
    let mut bytes = Vec::new();
    for l in String::from_utf8_lossy(&out.stdout).lines() {
        let Some(rest) = l.trim_start().strip_prefix("0x") else { continue };
        // `0x<addr> <up to four 8-digit groups> <ascii>`.
        for word in rest.split_whitespace().skip(1).take(4) {
            if word.len() % 2 != 0 || !word.bytes().all(|b| b.is_ascii_hexdigit()) {
                break;
            }
            for i in (0..word.len()).step_by(2) {
                bytes.push(u8::from_str_radix(&word[i..i + 2], 16).unwrap());
            }
        }
    }
    (shape, rela, bytes)
}

/// Checks that `tail` is exactly hod's own unit: 88 bytes, producer 2,
/// one kind 1 block with the one row `reloc-model` (hod's spec/codegen.md
/// §9.7.5).
fn assert_hods_unit(tail: &[u8], what: &str) {
    assert_eq!(tail.len(), 88, "{what}: hod's unit is 88 bytes");
    let u16_at = |o: usize| u16::from_le_bytes([tail[o], tail[o + 1]]);
    let u32_at = |o: usize| u32::from_le_bytes(tail[o..o + 4].try_into().unwrap());
    assert_eq!(&tail[..4], b"MASN", "{what}");
    assert_eq!((u32_at(8), u32_at(16), u16_at(28)), (88, 1, 2), "{what}: size, blocks, producer");
    assert_eq!(kinds(tail), [1], "{what}");
    let strtab = &tail[u32_at(20) as usize..][..u32_at(24) as usize];
    assert!(strtab.starts_with(b"\0reloc-model\0"), "{what}: {strtab:?}");
}

/// The record kinds of a unit's blocks (MT-10, MT-11: a 32-byte header
/// with `block_count` at 16, then blocks `{u16 kind; u16 version; u32
/// block_size; ...}`).
fn kinds(unit: &[u8]) -> Vec<u16> {
    let u32_at = |o: usize| u32::from_le_bytes(unit[o..o + 4].try_into().unwrap());
    assert_eq!(&unit[..4], b"MASN");
    let mut at = 32;
    (0..u32_at(16))
        .map(|_| {
            let kind = u16::from_le_bytes([unit[at], unit[at + 1]]);
            at += u32_at(at + 4) as usize;
            kind
        })
        .collect()
}

#[test]
fn the_section_is_the_llvm_routes_through_hod() {
    let Some(llvm) = llvm_dir() else { return };
    let readelf = llvm.join("bin/llvm-readelf");
    let hod = std::env::var_os("KILN_HOD_EXTERNAL").filter(|p| !p.is_empty()).map(PathBuf::from);
    if hod.is_none() {
        eprintln!("skipping the external route: KILN_HOD_EXTERNAL is not set");
    }
    let dir = std::env::temp_dir().join(format!("kiln-hod-mason-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let inputs: [(&str, &str, &[&str]); 6] = [
        ("t.cpp", CXX, &["-fmason-metadata"]),
        ("t.c", C, &["-fmason-metadata"]),
        ("decls.cpp", DECLS_CXX, &["-fmason-metadata=all"]),
        ("decls_c.c", DECLS_C, &["-fmason-metadata=all", "-std=c17"]),
        ("every.cpp", EVERY_KIND, &["-fmason-metadata=all"]),
        ("every.cpp", EVERY_KIND, &["-fmason-metadata=all", "-g"]),
    ];
    let mut routes = vec![Route::InProcess];
    if hod.is_some() {
        routes.push(Route::External);
    }
    for (file, src, args) in inputs {
        std::fs::write(dir.join(file), src).unwrap();
        compile(&dir, file, args, Route::Llvm, None, "llvm.o");
        let reference = section(&readelf, &dir.join("llvm.o"));
        assert_eq!(reference.0[0], "PROGBITS", "{file}: {:?}", reference.0);
        assert_eq!(reference.0[3], "E", "{file}: flags exactly SHF_EXCLUDE: {:?}", reference.0);
        assert_eq!(reference.0[6], "8", "{file}: alignment: {:?}", reference.0);
        assert!(!reference.1, "{file}: a relocation section applies to .mason.meta");
        assert_eq!(
            usize::from_str_radix(&reference.0[1], 16).unwrap(),
            reference.2.len(),
            "{file}: the dump's size"
        );
        if file == "every.cpp" {
            assert_eq!(kinds(&reference.2), [1, 2, 3, 4, 5], "{file}: every record kind");
        }
        for &route in &routes {
            compile(&dir, file, args, route, hod.as_deref(), "hod.o");
            let what = format!("{file} {args:?} {route:?}");
            let mut got = section(&readelf, &dir.join("hod.o"));
            // The size is the one header field hod's unit changes.
            let size = usize::from_str_radix(&got.0[1], 16).unwrap();
            assert_eq!(size, got.2.len(), "{what}: the dump's size");
            got.0[1].clone_from(&reference.0[1]);
            assert_eq!(got.0, reference.0, "{what}: the section header");
            assert_eq!(got.1, reference.1, "{what}: relocations");
            let (kiln, tail) = got.2.split_at(reference.2.len().min(got.2.len()));
            assert!(kiln == reference.2.as_slice(), "{what}: kiln's unit is not first or differs");
            assert_hods_unit(tail, &what);
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
}
