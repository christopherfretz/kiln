// clang's `unavailable` attribute (WP-954; clang `UnavailableAttr`,
// [dcl.attr.grammar]/5 implementation-defined attributes): a use of the
// entity is an error with its optional string-literal message and a
// `marked unavailable here` note at the redeclaration that carries the
// attribute, which later redeclarations inherit (f). A use inside an
// unavailable function is no error (ok). It takes at most one argument
// (clang names it without its scope), which is a string literal.
//@ std: c++20
//@ expect: compile-fail
//@ strict-diags
//@ diag: 36:3 error "'f' is unavailable"
//@ diag: 27:6 note "'f' has been explicitly marked unavailable here"
//@ diag: 37:9 error "'v' is unavailable: use w"
//@ diag: 28:5 note "'v' has been explicitly marked unavailable here"
//@ diag: 38:3 error "'S' is unavailable"
//@ diag: 29:37 note "'S' has been explicitly marked unavailable here"
//@ diag: 40:9 error "'B' is unavailable: gone"
//@ diag: 30:13 note "'B' has been explicitly marked unavailable here"
//@ diag: 41:7 error "'m' is unavailable: no m"
//@ diag: 32:8 note "'m' has been explicitly marked unavailable here"
//@ diag: 42:12 error "'n' is unavailable"
//@ diag: 33:37 note "'n' has been explicitly marked unavailable here"
//@ diag: 48:26 error "'unavailable' attribute takes no more than 1 argument"
//@ diag: 49:38 error "expected string literal as argument of 'unavailable' attribute"
//@ diag: 50:32 error "expected string literal as argument of 'unavailable' attribute"
void f() __attribute__((unavailable));
void f();
int v __attribute__((unavailable("use w")));
struct __attribute__((unavailable)) S {};
enum E { A, B __attribute__((unavailable("gone"))) };
struct T {
  void m() __attribute__((unavailable("no m")));
  [[clang::unavailable]] static int n;
};
void g() {
  f();
  (void)v;
  S *p = nullptr;
  (void)p;
  (void)B;
  T().m();
  (void)T::n;
}
void ok() __attribute__((unavailable)) {
  f();
  (void)v;
}
void h1() __attribute__((unavailable("a", "b")));
void h2() __attribute__((unavailable(1)));
void h3() [[clang::unavailable(0)]];
