// The spellings of `warn_unused_result` together, as clang 19 diagnoses
// them: the argument (only `[[nodiscard]]` keeps the message), every
// non-standard spelling named 'warn_unused_result', the return type's
// attribute before the callee's, a temporary diagnosed only for the
// `[[nodiscard]]` spelling. The clang 22 sibling is
// warn-unused-result-spellings-clang22.cpp. spec/sema.md §4.5, WP-1837.
//@ verify
//@ requires: clang<22
namespace args {
__attribute__((warn_unused_result(1))) int g1(); // expected-error {{expected string literal as argument of 'warn_unused_result' attribute}}
[[gnu::warn_unused_result(1)]] int g2(); // expected-error {{expected string literal as argument of 'warn_unused_result' attribute}}
[[clang::warn_unused_result(1)]] int g3(); // expected-error {{expected string literal as argument of 'warn_unused_result' attribute}}
[[nodiscard(1)]] int g4(); // expected-error {{expected string literal as argument of 'nodiscard' attribute}}
__attribute__((warn_unused_result("a", "b"))) int g5(); // expected-error {{'warn_unused_result' attribute takes no more than 1 argument}}
[[clang::warn_unused_result("a", "b")]] int g6(); // expected-error {{'warn_unused_result' attribute takes no more than 1 argument}}
[[clang::warn_unused_result()]] int g7();
[[clang::warn_unused_result, clang::warn_unused_result]] int g10();
[[clang::warn_unused_result]] [[nodiscard("z")]] int g11();
[[nodiscard("z")]] [[clang::warn_unused_result("y")]] int g12();
int g13();
[[clang::warn_unused_result("late")]] int g13();
void u() {
  g1();
  g2();
  g3();
  g4();
  g5();
  g6();
  g7(); // expected-warning {{ignoring return value of function declared with 'warn_unused_result' attribute}}
  g10(); // expected-warning {{ignoring return value of function declared with 'warn_unused_result' attribute}}
  g11(); // expected-warning {{ignoring return value of function declared with 'warn_unused_result' attribute}}
  g12(); // expected-warning {{ignoring return value of function declared with 'nodiscard' attribute: z}}
  g13(); // expected-warning {{ignoring return value of function declared with 'warn_unused_result' attribute}}
}
}
namespace names {
__attribute__((__warn_unused_result__)) int f1();
[[gnu::__warn_unused_result__]] int f2();
[[__gnu__::warn_unused_result]] int f3();
[[_Clang::warn_unused_result]] int f4();
[[clang::__warn_unused_result__]] int f5();
struct [[nodiscard("t")]] S {};
[[clang::warn_unused_result("c")]] S f6();
[[nodiscard("c")]] S f7();
typedef int T [[clang::warn_unused_result("td")]];
[[nodiscard("c")]] T f8();
void u() {
  f1(); // expected-warning {{ignoring return value of function declared with 'warn_unused_result' attribute}}
  f2(); // expected-warning {{ignoring return value of function declared with 'warn_unused_result' attribute}}
  f3(); // expected-warning {{ignoring return value of function declared with 'warn_unused_result' attribute}}
  f4(); // expected-warning {{ignoring return value of function declared with 'warn_unused_result' attribute}}
  f5(); // expected-warning {{ignoring return value of function declared with 'warn_unused_result' attribute}}
  f6(); // expected-warning {{ignoring return value of function declared with 'nodiscard' attribute: t}}
  f7(); // expected-warning {{ignoring return value of function declared with 'nodiscard' attribute: t}}
  f8(); // expected-warning {{ignoring return value of function declared with 'warn_unused_result' attribute}}
}
}
namespace qualified {
namespace N {
struct [[nodiscard]] S { S(int); };
enum class [[nodiscard]] E { a };
template<class T> struct [[nodiscard]] W { W(int); };
typedef int T [[clang::warn_unused_result]];
template<class X> struct Y {
  typedef X U [[clang::warn_unused_result]];
  static X (*fp)() [[clang::warn_unused_result]]; // expected-error {{'warn_unused_result' attribute cannot be applied to types}}
  [[nodiscard]] static X (*fq)(); // expected-warning {{'nodiscard' attribute only applies to functions, classes, or enumerations}}
};
}
N::S f1();
N::T f2();
N::Y<int>::U f3();
typedef N::T T2;
T2 f4();
const N::T f5();
N::E fe();
N::W<int> fw();
void u() {
  f1(); // expected-warning {{ignoring return value of function declared with 'nodiscard' attribute}}
  f2(); // expected-warning {{ignoring return value of function declared with 'warn_unused_result' attribute}}
  f3(); // expected-warning {{ignoring return value of function declared with 'warn_unused_result' attribute}}
  f4(); // expected-warning {{ignoring return value of function declared with 'warn_unused_result' attribute}}
  f5(); // expected-warning {{ignoring return value of function declared with 'warn_unused_result' attribute}}
  N::S(1); // expected-warning {{ignoring temporary created by a constructor declared with 'nodiscard' attribute}}
  N::W<int>(1); // expected-warning {{ignoring temporary created by a constructor declared with 'nodiscard' attribute}}
  fe(); // expected-warning {{ignoring return value of function declared with 'nodiscard' attribute}}
  fw(); // expected-warning {{ignoring return value of function declared with 'nodiscard' attribute}}
  N::Y<int>::fp();
  N::Y<int>::fq(); // expected-warning {{ignoring return value of function declared with 'nodiscard' attribute}}
}
}
namespace typedefs {
typedef int T __attribute__((warn_unused_result));
T mkt();
using U __attribute__((warn_unused_result)) = int;
U mku();
typedef T T4;
T4 mkt4();
struct C { [[nodiscard]] C(int); C(int, int); };
struct [[nodiscard]] D { D(int); };
typedef int (*FP)() __attribute__((warn_unused_result));
FP gfp;
int (*fp2)() __attribute__((warn_unused_result));
[[nodiscard]] int (*fp3)(); // expected-warning {{'nodiscard' attribute only applies to functions, classes, or enumerations}}
[[clang::warn_unused_result]] int &rf();
typedef int T3 [[nodiscard]]; // expected-warning {{'[[nodiscard]]' attribute ignored when applied to a typedef; consider using '__attribute__((warn_unused_result))' or '[[clang::warn_unused_result]]' instead}}
void use() {
  mkt(); // expected-warning {{ignoring return value of function declared with 'warn_unused_result' attribute}}
  mku(); // expected-warning {{ignoring return value of function declared with 'warn_unused_result' attribute}}
  mkt4(); // expected-warning {{ignoring return value of function declared with 'warn_unused_result' attribute}}
  C(1); // expected-warning {{ignoring temporary created by a constructor declared with 'nodiscard' attribute}}
  C(1, 2);
  D(1); // expected-warning {{ignoring temporary created by a constructor declared with 'nodiscard' attribute}}
  D{1}; // expected-warning {{ignoring temporary created by a constructor declared with 'nodiscard' attribute}}
  gfp();
  fp2(); // expected-warning {{ignoring return value of function declared with 'warn_unused_result' attribute}}
  fp3(); // expected-warning {{ignoring return value of function declared with 'nodiscard' attribute}}
  rf(); // expected-warning {{ignoring return value of function declared with 'warn_unused_result' attribute}}
}
}
namespace refs {
struct [[nodiscard]] S {};
typedef S &SR [[clang::warn_unused_result]];
typedef int I [[clang::warn_unused_result]];
typedef int &Q [[clang::warn_unused_result]];
I &h();
S &k();
SR j();
Q q();
struct C { I &m(); SR n(); };
auto l = []() -> S { return {}; };
void w(C c) {
  h();
  k();
  j(); // expected-warning {{ignoring return value of function declared with 'warn_unused_result' attribute}}
  q(); // expected-warning {{ignoring return value of function declared with 'warn_unused_result' attribute}}
  c.m();
  c.n(); // expected-warning {{ignoring return value of function declared with 'warn_unused_result' attribute}}
  l(); // expected-warning {{ignoring return value of function declared with 'nodiscard' attribute}}
}
}
