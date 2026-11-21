// `[[clang::warn_unused_result]]` is clang's scoped spelling of GNU
// `warn_unused_result` (C++ only; in C it is an unknown attribute), and
// behaves like `__attribute__((warn_unused_result))`,
// `[[gnu::warn_unused_result]]` and `[[_Clang::__warn_unused_result__]]`
// on every subject: a function, a class or enumeration returned by value,
// a typedef returned (not `[[gnu::]]` on a typedef), a constructor, and
// not a variable or a function returning `void`. clang 22 names the
// spelling as written, takes the message of the `clang::` spellings,
// warns for every spelling on a discarded temporary and names the type
// on the type path [-Wunused-value]. The clang 19 behaviour is in the
// `-clang19` sibling. spec/sema.md §4.5, WP-1837.
//@ verify
//@ requires: clang>=22
namespace cl {
[[clang::warn_unused_result]] int f1();
[[clang::warn_unused_result("m")]] int f2();
struct [[clang::warn_unused_result]] S { S(); S(int); S(int, int); int x; };
S mk();
S &mkr();
struct P { [[clang::warn_unused_result]] P(int); [[clang::warn_unused_result]] P(int, int); [[clang::warn_unused_result("m")]] P(long); P(); };
enum [[clang::warn_unused_result]] E { e0 };
E mke();
enum class [[clang::warn_unused_result("m")]] EC { x };
EC mkec();
typedef int T [[clang::warn_unused_result]];
T mkt();
typedef int TM [[clang::warn_unused_result("m")]];
TM mktm();
[[clang::warn_unused_result]] void v(); // expected-warning {{attribute 'clang::warn_unused_result' cannot be applied to functions without return value}}
[[clang::warn_unused_result]] int var; // expected-warning {{'clang::warn_unused_result' attribute only applies to Objective-C methods, enums, structs, unions, classes, functions, function pointers, and typedefs}}
[[clang::warn_unused_result]] int (*fp)();
struct M { int m [[clang::warn_unused_result]]; [[clang::warn_unused_result]] int g(); [[clang::warn_unused_result]] static int sg(); }; // expected-warning {{'clang::warn_unused_result' attribute only applies to Objective-C methods, enums, structs, unions, classes, functions, function pointers, and typedefs}}
struct V { [[clang::warn_unused_result]] virtual int h(); };
template<class X> [[clang::warn_unused_result]] X tf(X);
void use(M mm, V &vv) {
  f1(); // expected-warning {{ignoring return value of function declared with 'clang::warn_unused_result' attribute}}
  f2(); // expected-warning {{ignoring return value of function declared with 'clang::warn_unused_result' attribute: m}}
  mk(); // expected-warning {{ignoring return value of type 'S' declared with 'clang::warn_unused_result' attribute}}
  mkr();
  mke(); // expected-warning {{ignoring return value of type 'E' declared with 'clang::warn_unused_result' attribute}}
  mkec(); // expected-warning {{ignoring return value of type 'EC' declared with 'clang::warn_unused_result' attribute: m}}
  mkt(); // expected-warning {{ignoring return value of type 'T' declared with 'clang::warn_unused_result' attribute}}
  mktm(); // expected-warning {{ignoring return value of type 'TM' declared with 'clang::warn_unused_result' attribute: m}}
  fp(); // expected-warning {{ignoring return value of function declared with 'clang::warn_unused_result' attribute}}
  S(1); // expected-warning {{ignoring temporary of type 'S' declared with 'clang::warn_unused_result' attribute}}
  S{1}; // expected-warning {{ignoring temporary of type 'S' declared with 'clang::warn_unused_result' attribute}}
  S(1, 2); // expected-warning {{ignoring temporary of type 'S' declared with 'clang::warn_unused_result' attribute}}
  S(); // expected-warning {{ignoring temporary of type 'S' declared with 'clang::warn_unused_result' attribute}}
  P(1); // expected-warning {{ignoring temporary created by a constructor declared with 'clang::warn_unused_result' attribute}}
  P{1}; // expected-warning {{ignoring temporary created by a constructor declared with 'clang::warn_unused_result' attribute}}
  P(1, 2); // expected-warning {{ignoring temporary created by a constructor declared with 'clang::warn_unused_result' attribute}}
  P(1L); // expected-warning {{ignoring temporary created by a constructor declared with 'clang::warn_unused_result' attribute: m}}
  P();
  mm.g(); // expected-warning {{ignoring return value of function declared with 'clang::warn_unused_result' attribute}}
  M::sg(); // expected-warning {{ignoring return value of function declared with 'clang::warn_unused_result' attribute}}
  vv.h(); // expected-warning {{ignoring return value of function declared with 'clang::warn_unused_result' attribute}}
  tf(1); // expected-warning {{ignoring return value of function declared with 'clang::warn_unused_result' attribute}}
  (void)f1();
  static_cast<void>(f2());
  (void)S(1);
}
}

namespace gnu {
__attribute__((warn_unused_result)) int f1();
__attribute__((warn_unused_result("m"))) int f2();
struct __attribute__((warn_unused_result)) S { S(); S(int); S(int, int); int x; };
S mk();
S &mkr();
struct P { __attribute__((warn_unused_result)) P(int); __attribute__((warn_unused_result)) P(int, int); __attribute__((warn_unused_result("m"))) P(long); P(); };
enum __attribute__((warn_unused_result)) E { e0 };
E mke();
enum class __attribute__((warn_unused_result("m"))) EC { x };
EC mkec();
typedef int T __attribute__((warn_unused_result));
T mkt();
typedef int TM __attribute__((warn_unused_result("m")));
TM mktm();
__attribute__((warn_unused_result)) void v(); // expected-warning {{attribute 'warn_unused_result' cannot be applied to functions without return value}}
__attribute__((warn_unused_result)) int var; // expected-warning {{'warn_unused_result' attribute only applies to Objective-C methods, enums, structs, unions, classes, functions, function pointers, and typedefs}}
__attribute__((warn_unused_result)) int (*fp)();
struct M { int m __attribute__((warn_unused_result)); __attribute__((warn_unused_result)) int g(); __attribute__((warn_unused_result)) static int sg(); }; // expected-warning {{'warn_unused_result' attribute only applies to Objective-C methods, enums, structs, unions, classes, functions, function pointers, and typedefs}}
struct V { __attribute__((warn_unused_result)) virtual int h(); };
template<class X> __attribute__((warn_unused_result)) X tf(X);
void use(M mm, V &vv) {
  f1(); // expected-warning {{ignoring return value of function declared with 'warn_unused_result' attribute}}
  f2(); // expected-warning {{ignoring return value of function declared with 'warn_unused_result' attribute}}
  mk(); // expected-warning {{ignoring return value of type 'S' declared with 'warn_unused_result' attribute}}
  mkr();
  mke(); // expected-warning {{ignoring return value of type 'E' declared with 'warn_unused_result' attribute}}
  mkec(); // expected-warning {{ignoring return value of type 'EC' declared with 'warn_unused_result' attribute}}
  mkt(); // expected-warning {{ignoring return value of type 'T' declared with 'warn_unused_result' attribute}}
  mktm(); // expected-warning {{ignoring return value of type 'TM' declared with 'warn_unused_result' attribute}}
  fp(); // expected-warning {{ignoring return value of function declared with 'warn_unused_result' attribute}}
  S(1); // expected-warning {{ignoring temporary of type 'S' declared with 'warn_unused_result' attribute}}
  S{1}; // expected-warning {{ignoring temporary of type 'S' declared with 'warn_unused_result' attribute}}
  S(1, 2); // expected-warning {{ignoring temporary of type 'S' declared with 'warn_unused_result' attribute}}
  S(); // expected-warning {{ignoring temporary of type 'S' declared with 'warn_unused_result' attribute}}
  P(1); // expected-warning {{ignoring temporary created by a constructor declared with 'warn_unused_result' attribute}}
  P{1}; // expected-warning {{ignoring temporary created by a constructor declared with 'warn_unused_result' attribute}}
  P(1, 2); // expected-warning {{ignoring temporary created by a constructor declared with 'warn_unused_result' attribute}}
  P(1L); // expected-warning {{ignoring temporary created by a constructor declared with 'warn_unused_result' attribute}}
  P();
  mm.g(); // expected-warning {{ignoring return value of function declared with 'warn_unused_result' attribute}}
  M::sg(); // expected-warning {{ignoring return value of function declared with 'warn_unused_result' attribute}}
  vv.h(); // expected-warning {{ignoring return value of function declared with 'warn_unused_result' attribute}}
  tf(1); // expected-warning {{ignoring return value of function declared with 'warn_unused_result' attribute}}
  (void)f1();
  static_cast<void>(f2());
  (void)S(1);
}
}

namespace gs {
[[gnu::warn_unused_result]] int f1();
[[gnu::warn_unused_result("m")]] int f2();
struct [[gnu::warn_unused_result]] S { S(); S(int); S(int, int); int x; };
S mk();
S &mkr();
struct P { [[gnu::warn_unused_result]] P(int); [[gnu::warn_unused_result]] P(int, int); [[gnu::warn_unused_result("m")]] P(long); P(); };
enum [[gnu::warn_unused_result]] E { e0 };
E mke();
enum class [[gnu::warn_unused_result("m")]] EC { x };
EC mkec();
typedef int T [[gnu::warn_unused_result]]; // expected-warning {{'[[gnu::warn_unused_result]]' attribute ignored when applied to a typedef; consider using '__attribute__((warn_unused_result))' or '[[clang::warn_unused_result]]' instead}}
T mkt();
typedef int TM [[gnu::warn_unused_result("m")]]; // expected-warning {{'[[gnu::warn_unused_result]]' attribute ignored when applied to a typedef; consider using '__attribute__((warn_unused_result))' or '[[clang::warn_unused_result]]' instead}}
TM mktm();
[[gnu::warn_unused_result]] void v(); // expected-warning {{attribute 'gnu::warn_unused_result' cannot be applied to functions without return value}}
[[gnu::warn_unused_result]] int var; // expected-warning {{'gnu::warn_unused_result' attribute only applies to Objective-C methods, enums, structs, unions, classes, functions, function pointers, and typedefs}}
[[gnu::warn_unused_result]] int (*fp)();
struct M { int m [[gnu::warn_unused_result]]; [[gnu::warn_unused_result]] int g(); [[gnu::warn_unused_result]] static int sg(); }; // expected-warning {{'gnu::warn_unused_result' attribute only applies to Objective-C methods, enums, structs, unions, classes, functions, function pointers, and typedefs}}
struct V { [[gnu::warn_unused_result]] virtual int h(); };
template<class X> [[gnu::warn_unused_result]] X tf(X);
void use(M mm, V &vv) {
  f1(); // expected-warning {{ignoring return value of function declared with 'gnu::warn_unused_result' attribute}}
  f2(); // expected-warning {{ignoring return value of function declared with 'gnu::warn_unused_result' attribute}}
  mk(); // expected-warning {{ignoring return value of type 'S' declared with 'gnu::warn_unused_result' attribute}}
  mkr();
  mke(); // expected-warning {{ignoring return value of type 'E' declared with 'gnu::warn_unused_result' attribute}}
  mkec(); // expected-warning {{ignoring return value of type 'EC' declared with 'gnu::warn_unused_result' attribute}}
  mkt();
  mktm();
  fp(); // expected-warning {{ignoring return value of function declared with 'gnu::warn_unused_result' attribute}}
  S(1); // expected-warning {{ignoring temporary of type 'S' declared with 'gnu::warn_unused_result' attribute}}
  S{1}; // expected-warning {{ignoring temporary of type 'S' declared with 'gnu::warn_unused_result' attribute}}
  S(1, 2); // expected-warning {{ignoring temporary of type 'S' declared with 'gnu::warn_unused_result' attribute}}
  S(); // expected-warning {{ignoring temporary of type 'S' declared with 'gnu::warn_unused_result' attribute}}
  P(1); // expected-warning {{ignoring temporary created by a constructor declared with 'gnu::warn_unused_result' attribute}}
  P{1}; // expected-warning {{ignoring temporary created by a constructor declared with 'gnu::warn_unused_result' attribute}}
  P(1, 2); // expected-warning {{ignoring temporary created by a constructor declared with 'gnu::warn_unused_result' attribute}}
  P(1L); // expected-warning {{ignoring temporary created by a constructor declared with 'gnu::warn_unused_result' attribute}}
  P();
  mm.g(); // expected-warning {{ignoring return value of function declared with 'gnu::warn_unused_result' attribute}}
  M::sg(); // expected-warning {{ignoring return value of function declared with 'gnu::warn_unused_result' attribute}}
  vv.h(); // expected-warning {{ignoring return value of function declared with 'gnu::warn_unused_result' attribute}}
  tf(1); // expected-warning {{ignoring return value of function declared with 'gnu::warn_unused_result' attribute}}
  (void)f1();
  static_cast<void>(f2());
  (void)S(1);
}
}

namespace sc {
[[_Clang::__warn_unused_result__]] int f1();
[[_Clang::__warn_unused_result__("m")]] int f2();
struct [[_Clang::__warn_unused_result__]] S { S(); S(int); S(int, int); int x; };
S mk();
S &mkr();
struct P { [[_Clang::__warn_unused_result__]] P(int); [[_Clang::__warn_unused_result__]] P(int, int); [[_Clang::__warn_unused_result__("m")]] P(long); P(); };
enum [[_Clang::__warn_unused_result__]] E { e0 };
E mke();
enum class [[_Clang::__warn_unused_result__("m")]] EC { x };
EC mkec();
typedef int T [[_Clang::__warn_unused_result__]];
T mkt();
typedef int TM [[_Clang::__warn_unused_result__("m")]];
TM mktm();
[[_Clang::__warn_unused_result__]] void v(); // expected-warning {{attribute 'clang::warn_unused_result' cannot be applied to functions without return value}}
[[_Clang::__warn_unused_result__]] int var; // expected-warning {{'clang::warn_unused_result' attribute only applies to Objective-C methods, enums, structs, unions, classes, functions, function pointers, and typedefs}}
[[_Clang::__warn_unused_result__]] int (*fp)();
struct M { int m [[_Clang::__warn_unused_result__]]; [[_Clang::__warn_unused_result__]] int g(); [[_Clang::__warn_unused_result__]] static int sg(); }; // expected-warning {{'clang::warn_unused_result' attribute only applies to Objective-C methods, enums, structs, unions, classes, functions, function pointers, and typedefs}}
struct V { [[_Clang::__warn_unused_result__]] virtual int h(); };
template<class X> [[_Clang::__warn_unused_result__]] X tf(X);
void use(M mm, V &vv) {
  f1(); // expected-warning {{ignoring return value of function declared with '_Clang::warn_unused_result' attribute}}
  f2(); // expected-warning {{ignoring return value of function declared with '_Clang::warn_unused_result' attribute: m}}
  mk(); // expected-warning {{ignoring return value of type 'S' declared with '_Clang::warn_unused_result' attribute}}
  mkr();
  mke(); // expected-warning {{ignoring return value of type 'E' declared with '_Clang::warn_unused_result' attribute}}
  mkec(); // expected-warning {{ignoring return value of type 'EC' declared with '_Clang::warn_unused_result' attribute: m}}
  mkt(); // expected-warning {{ignoring return value of type 'T' declared with '_Clang::warn_unused_result' attribute}}
  mktm(); // expected-warning {{ignoring return value of type 'TM' declared with '_Clang::warn_unused_result' attribute: m}}
  fp(); // expected-warning {{ignoring return value of function declared with '_Clang::warn_unused_result' attribute}}
  S(1); // expected-warning {{ignoring temporary of type 'S' declared with '_Clang::warn_unused_result' attribute}}
  S{1}; // expected-warning {{ignoring temporary of type 'S' declared with '_Clang::warn_unused_result' attribute}}
  S(1, 2); // expected-warning {{ignoring temporary of type 'S' declared with '_Clang::warn_unused_result' attribute}}
  S(); // expected-warning {{ignoring temporary of type 'S' declared with '_Clang::warn_unused_result' attribute}}
  P(1); // expected-warning {{ignoring temporary created by a constructor declared with '_Clang::warn_unused_result' attribute}}
  P{1}; // expected-warning {{ignoring temporary created by a constructor declared with '_Clang::warn_unused_result' attribute}}
  P(1, 2); // expected-warning {{ignoring temporary created by a constructor declared with '_Clang::warn_unused_result' attribute}}
  P(1L); // expected-warning {{ignoring temporary created by a constructor declared with '_Clang::warn_unused_result' attribute: m}}
  P();
  mm.g(); // expected-warning {{ignoring return value of function declared with '_Clang::warn_unused_result' attribute}}
  M::sg(); // expected-warning {{ignoring return value of function declared with '_Clang::warn_unused_result' attribute}}
  vv.h(); // expected-warning {{ignoring return value of function declared with '_Clang::warn_unused_result' attribute}}
  tf(1); // expected-warning {{ignoring return value of function declared with '_Clang::warn_unused_result' attribute}}
  (void)f1();
  static_cast<void>(f2());
  (void)S(1);
}
}
