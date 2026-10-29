//! End-to-end tests of class objects in the Oven (WP-076, spec/oven.md
//! "Classes"): member pointer values frozen and thawed across constexpr
//! variables, deep expression spines (the iterative walkers), static
//! temporaries frozen into `LetData.value`, constant destruction and the
//! counters of class construction.
#![forbid(unsafe_code)]

use kiln_diag::SharedBuffer;
use kiln_driver::{Io, SharedWriter, run};

struct Out {
    code: i32,
    stderr: String,
}

fn kiln(args: &[&str], src: &str) -> Out {
    let err = SharedBuffer::new();
    let io = Io {
        stdout: Box::new(SharedBuffer::new()),
        stderr: SharedWriter::new(err.clone()),
        stdin: Box::new(std::io::Cursor::new(src.as_bytes().to_vec())),
    };
    let mut owned: Vec<String> = vec!["-std=c++26".into(), "-fsyntax-only".into()];
    owned.extend(args.iter().map(|s| (*s).to_owned()));
    owned.push("-".into());
    let code = run(&owned, io);
    Out { code, stderr: err.contents() }
}

#[track_caller]
fn accepts(src: &str) {
    let o = kiln(&[], src);
    assert_eq!((o.code, o.stderr.as_str()), (0, ""), "\nsource:\n{src}");
}

/// [conv.mem]/2, [expr.static.cast]/12, [expr.eq]/4: a member pointer
/// converted to a derived class, frozen into a constexpr variable, thawed
/// and cast back compares equal to the original (same member, same path);
/// the null value round-trips; both reach the member.
#[test]
fn member_pointer_intern_cast_round_trip() {
    accepts(
        "\
struct A { int a; };
struct B { int b; constexpr int get() const { return b; } };
struct D : A, B { int d; };
constexpr int B::*pb = &B::b;
constexpr int D::*pd = pb;
constexpr int B::*back = static_cast<int B::*>(pd);
static_assert(back == pb && pd == &D::b && pd != &D::d);
constexpr int (B::*fb)() const = &B::get;
constexpr int (D::*fd)() const = fb;
static_assert(static_cast<int (B::*)() const>(fd) == fb);
constexpr int D::*nul = static_cast<int B::*>(nullptr);
static_assert(nul == nullptr && static_cast<int B::*>(nul) == nullptr);
constexpr int use() { D x{}; x.b = 4; x.d = 5; const B &rb = x; return x.*pd * 100 + rb.*back * 10 + (x.*fd)(); }
static_assert(use() == 444);
",
    );
}

/// R-WALK (spec/oven.md): arithmetic, comma and logical spines of 20000
/// operands lower without recursion in the Oven's compiler.
#[test]
fn deep_spines() {
    let n = 20000;
    let ones = vec!["1"; n].join("+");
    let gs = vec!["g"; n].join(",");
    let ts = vec!["true"; n].join("&&");
    let xs = vec!["x"; n].join("-");
    let src = format!(
        "static_assert({ones} == {n});\n\
         constexpr int g = 0;\n\
         static_assert(({gs}, true));\n\
         static_assert({ts});\n\
         constexpr int f(int x) {{ return {xs}; }}\n\
         static_assert(f(1) == {});\n",
        2 - n as i64
    );
    // Parse and sema may recurse along the spines: give them room.
    let h = std::thread::Builder::new()
        .stack_size(256 << 20)
        .spawn(move || kiln(&[], &src))
        .expect("thread");
    let o = h.join().expect("join");
    assert_eq!((o.code, o.stderr.as_str()), (0, ""));
}

/// [class.temporary]/6, [expr.const]/4: temporaries extended by constexpr
/// references are frozen with the reference's value; const ones are
/// readable, pointers to them are constant and stable.
#[test]
fn static_temporaries() {
    accepts(
        "\
struct S { int a, b; constexpr int sum() const { return a + b; } };
constexpr const int &r = 5;
constexpr const int *p = &r, *q = &r;
static_assert(r == 5 && *p == 5 && p == q);
constexpr const S &s = S{1, 2};
static_assert(s.sum() == 3);
struct N { const S &s; int k; };
constexpr N n{S{3, 4}, 5};
static_assert(n.s.b == 4 && n.k == 5);
const int &c = 6;
constexpr int g = c;
",
    );
    let o = kiln(&[], "struct S { int a; };\nconstexpr S &&t = S{3};\nstatic_assert(t.a == 3);\n");
    assert_eq!(o.code, 1);
    assert!(
        o.stderr.contains("read of temporary is not allowed in a constant expression outside the expression that created the temporary"),
        "{}",
        o.stderr
    );
}

/// [dcl.constexpr]/6: a constexpr variable's destruction must be a
/// constant expression; the destructor may modify the object.
#[test]
fn constant_destruction() {
    accepts(
        "struct M { int n; constexpr ~M() { n = 7; } };\nconstexpr M m{1};\nstatic_assert(m.n == 1);\n",
    );
    let o =
        kiln(&[], "int g;\nstruct A { int n; constexpr ~A() { g = n; } };\nconstexpr A a{1};\n");
    assert_eq!(o.code, 1);
    assert!(
        o.stderr.contains("constexpr variable 'a' must have constant destruction"),
        "{}",
        o.stderr
    );
    assert!(o.stderr.contains("in call to 'a.~A()'"), "{}", o.stderr);
}

/// spec/oven.md "Counters": class objects on the heap count as
/// allocations; their storage is reclaimed by `delete`.
#[test]
fn class_counters() {
    let dir = std::env::temp_dir().join(format!("kiln-oven-classes-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let report = dir.join("report.json");
    let arg = format!("-fconstexpr-report={}", report.display());
    let o = kiln(
        &[&arg],
        "\
struct T { int *p; constexpr T(int v) : p(new int(v)) {} constexpr ~T() { delete p; } };
constexpr int f(int n) {
  int s = 0;
  for (int i = 0; i < n; ++i) { T *t = new T(i); s += *t->p; delete t; }
  return s;
}
static_assert(f(100) == 4950);
",
    );
    assert_eq!((o.code, o.stderr.as_str()), (0, ""));
    let text = std::fs::read_to_string(&report).unwrap();
    std::fs::remove_file(&report).unwrap();
    std::fs::remove_dir(&dir).unwrap();
    let total = text.lines().last().unwrap();
    assert!(total.contains("\"allocs\":200,"), "{total}");
    assert!(
        total.contains("\"peak_bytes\":16,") || total.contains("\"peak_bytes\":24,"),
        "{total}"
    );
}
