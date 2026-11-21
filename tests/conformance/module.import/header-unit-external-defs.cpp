// [module.import]/6: a header unit shall not contain a definition of a
// non-inline function or variable whose name has external linkage. The
// test file is built as a header unit (`as-header-unit`). As clang's
// check, a function is checked at its body and a variable at its
// initializer: an internal, inline, constexpr or extern entity, a
// declaration and a variable with no initializer are not diagnosed;
// explicit specializations, conversion functions, destructors, `main`,
// `extern "C"` and inline-namespace members are.
//@ expect: compile-fail
//@ std: c++20
//@ strict-diags
//@ as-header-unit
//@ args: -ferror-limit=0
//@ diag: 44:5 error "non-inline external definitions are not permitted in C++ header units"
//@ diag: 48:5 error "non-inline external definitions are not permitted in C++ header units"
//@ diag: 49:5 error "non-inline external definitions are not permitted in C++ header units"
//@ diag: 50:5 error "non-inline external definitions are not permitted in C++ header units"
//@ diag: 51:5 error "non-inline external definitions are not permitted in C++ header units"
//@ diag: 52:5 error "non-inline external definitions are not permitted in C++ header units"
//@ diag: 53:5 error "non-inline external definitions are not permitted in C++ header units"
//@ diag: 56:15 error "non-inline external definitions are not permitted in C++ header units"
//@ diag: 57:18 error "non-inline external definitions are not permitted in C++ header units"
//@ diag: 58:6 error "non-inline external definitions are not permitted in C++ header units"
//@ diag: 59:6 error "non-inline external definitions are not permitted in C++ header units"
//@ diag: 60:16 error "non-inline external definitions are not permitted in C++ header units"
//@ diag: 61:18 error "non-inline external definitions are not permitted in C++ header units"
//@ diag: 63:5 error "non-inline external definitions are not permitted in C++ header units"
//@ diag: 64:26 error "non-inline external definitions are not permitted in C++ header units"
//@ diag: 70:3 error "non-inline external definitions are not permitted in C++ header units"
//@ diag: 71:6 error "non-inline external definitions are not permitted in C++ header units"
//@ diag: 80:8 error "non-inline external definitions are not permitted in C++ header units"
//@ diag: 81:14 error "non-inline external definitions are not permitted in C++ header units"
//@ diag: 82:4 error "non-inline external definitions are not permitted in C++ header units"
//@ diag: 83:4 error "non-inline external definitions are not permitted in C++ header units"
//@ diag: 84:9 error "non-inline external definitions are not permitted in C++ header units"
//@ diag: 86:17 error "non-inline external definitions are not permitted in C++ header units"
//@ diag: 88:26 error "non-inline external definitions are not permitted in C++ header units"
//@ diag: 89:19 error "non-inline external definitions are not permitted in C++ header units"
//@ diag: 90:20 warning "'weak_import' attribute cannot be specified on a definition"
//@ diag: 90:34 error "non-inline external definitions are not permitted in C++ header units"
//@ diag: 91:5 error "non-inline external definitions are not permitted in C++ header units"
//@ diag: 92:6 error "non-inline external definitions are not permitted in C++ header units"
//@ diag: 97:6 error "non-inline external definitions are not permitted in C++ header units"
int f() { return 1; }
inline int g() { return 2; }
static int s() { return 3; }
int f2();
int f2() { return 4; }
int v = 4;
int vi(1);
int vb{1};
int arr[] = {1};
int x1 = 0, x2;
static int sv = 6;
const int cv = 7;
constinit int ci = 1;
thread_local int tl = 1;
int &r = v;
auto lam = [] { return 1; };
extern "C" int cfun() { return 14; }
extern "C" { int cb = 1; }
namespace N {
int nf() { return 1; }
inline namespace I { int nif() { return 2; } }
} // namespace N
namespace {
struct L {};
int anon() { return 8; }
} // namespace
L lv = {};
void takes_l(L) {}
struct S {
  static int sm;
  static const int k2;
  S();
  ~S();
  operator int();
  void d();
};
int S::sm = 10;
const int S::k2 = 2;
S::~S() {}
S::operator int() { return 1; }
void S::d() {}
template <class T> int tf(T) { return 11; }
template <> int tf<char>(char) { return 12; }
template <class T> struct X { void f(); };
template <> void X<int>::f() {}
struct { int a; } anonv = {1};
int __attribute__((weak_import)) wi = 1;
int main() { return 0; }
auto af() { return 1; }
struct P {
  int a, b;
};
inline P pp{1, 2};
auto [b1, b2] = pp;
