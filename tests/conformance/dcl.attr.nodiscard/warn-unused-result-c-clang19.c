// `warn_unused_result` in C23 as clang 19 diagnoses it (the clang 22
// sibling is warn-unused-result-c-clang22.c): `clang::` is unknown in C,
// every return path is "function declared with".
// spec/sema.md §4.5, WP-1837.
//@ std: c23
//@ verify
//@ requires: clang<22
__attribute__((warn_unused_result)) int f1(void);
[[gnu::warn_unused_result]] int f2(void);
[[clang::warn_unused_result]] int f3(void); // expected-warning {{unknown attribute 'warn_unused_result' ignored}}
[[clang::warn_unused_result("cm")]] int f3m(void); // expected-warning {{unknown attribute 'warn_unused_result' ignored}}
[[nodiscard]] int f4(void);
[[nodiscard("nm")]] int f4m(void);
struct [[clang::warn_unused_result]] S { int x; }; // expected-warning {{unknown attribute 'warn_unused_result' ignored}}
struct S mk(void);
enum [[clang::warn_unused_result]] E { e0 }; // expected-warning {{unknown attribute 'warn_unused_result' ignored}}
enum E mke(void);
typedef int T [[clang::warn_unused_result]]; // expected-warning {{unknown attribute 'warn_unused_result' ignored}}
T mkt(void);
typedef int T2 [[gnu::warn_unused_result]]; // expected-warning {{'[[gnu::warn_unused_result]]' attribute ignored when applied to a typedef; consider using '__attribute__((warn_unused_result))' or '[[clang::warn_unused_result]]' instead}}
[[clang::warn_unused_result]] void v(void); // expected-warning {{unknown attribute 'warn_unused_result' ignored}}
[[clang::warn_unused_result]] int (*fp)(void); // expected-warning {{unknown attribute 'warn_unused_result' ignored}}
struct __attribute__((warn_unused_result)) S2 { int x; };
struct S2 mk2(void);
struct [[nodiscard("s3")]] S3 { int x; };
struct S3 mk3(void);
typedef int T4 __attribute__((warn_unused_result));
T4 mkt4(void);
[[gnu::warn_unused_result]] T4 mkt5(void);
void u(void) {
  mk2(); // expected-warning {{ignoring return value of function declared with 'warn_unused_result' attribute}}
  mk3(); // expected-warning {{ignoring return value of function declared with 'nodiscard' attribute: s3}}
  mkt4(); // expected-warning {{ignoring return value of function declared with 'warn_unused_result' attribute}}
  mkt5(); // expected-warning {{ignoring return value of function declared with 'warn_unused_result' attribute}}
  f1(); // expected-warning {{ignoring return value of function declared with 'warn_unused_result' attribute}}
  f2(); // expected-warning {{ignoring return value of function declared with 'warn_unused_result' attribute}}
  f3();
  f3m();
  f4(); // expected-warning {{ignoring return value of function declared with 'nodiscard' attribute}}
  f4m(); // expected-warning {{ignoring return value of function declared with 'nodiscard' attribute: nm}}
  mk();
  mke();
  mkt();
  fp();
  (void)f4();
}
