// [expr.const]/10, [class.default.ctor]/3: in C++23 the implicit default
// constructor of a class whose virtual bases are all indirect is constexpr;
// evaluating it reports the virtual base (clang 22, P2448 skips the literal
// type check). Clang 19 twin: constexpr-vbase-construct-cxx23-19.cpp.
//@ std: c++23
//@ expect: compile-fail
//@ strict-diags
//@ requires: clang>=22
//@ diag: 25:15 error "static assertion expression is not an integral constant expression"
//@ diag: 24:23 note "cannot construct object of type 'Y' with virtual base class in a constant expression"
//@ diag: 25:15 note "in call to 'a()'"
//@ diag: 28:15 error "static assertion expression is not an integral constant expression"
//@ diag: 27:23 note "cannot construct object of type 'Z' with virtual base class in a constant expression"
//@ diag: 28:15 note "in call to 'b()'"
//@ diag: 31:15 error "static assertion expression is not an integral constant expression"
//@ diag: 30:23 note "cannot construct object of type 'P' with virtual base class in a constant expression"
//@ diag: 31:15 note "in call to 'c()'"
//@ diag: 33:15 error "static assertion expression is not an integral constant expression"
//@ diag: 32:23 note "cannot construct object of type 'Y' with virtual base class in a constant expression"
//@ diag: 33:15 note "in call to 'd()'"
struct V { int v = 1; };
struct W : virtual V { };
struct Y : W {};
constexpr int a() { Y y; return 0; }
static_assert(a() == 0);
struct Z : W { Z() = default; };
constexpr int b() { Z z; return 0; }
static_assert(b() == 0);
struct P : private W { };
constexpr int c() { P z; return 0; }
static_assert(c() == 0);
constexpr int d() { Y y{}; return 0; }
static_assert(d() == 0);
