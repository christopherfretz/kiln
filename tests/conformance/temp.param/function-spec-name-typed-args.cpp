// [temp.arg.nontype]/1, [lex.icon], [dcl.fct.def.delete]/2,
// [dcl.attr.deprecated]: a function template specialization named in a
// diagnostic prints its own template arguments with their types (clang
// `FunctionDecl::getNameForDiagnostic` prints them without the template
// parameter list, so `shouldIncludeTypeForArgument` holds): `f<2U>`,
// `g<3UL, int>`, `d<9ULL>`; a nested class template-id keeps its
// parameters' rule (`h<X<4>>`), and the `[with N = 2]` bindings of a
// candidate note print bare. libc++ `.verify.cpp` deleted-function notes
// (WP-360, WP-366).
// Verified against clang 19.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 29:17 error "attempt to use a deleted function"
//@ diag: 23:28 note "'f<2U>' has been explicitly marked deleted here"
//@ diag: 30:20 error "attempt to use a deleted function"
//@ diag: 24:42 note "'g<3UL, int>' has been explicitly marked deleted here"
//@ diag: 31:17 error "attempt to use a deleted function"
//@ diag: 26:33 note "'h<X<4>, 5L>' has been explicitly marked deleted here"
//@ diag: 32:3 error "call to deleted function 'f'"
//@ diag: 23:28 note "candidate function [with N = 2] has been explicitly deleted"
//@ diag: 33:3 warning "'d<9ULL>' is deprecated"
//@ diag: 27:35 note "'d<9ULL>' has been explicitly marked deprecated here"
template <unsigned N> void f() = delete;
template <unsigned long N, class T> void g(T) = delete;
template <unsigned N> struct X {};
template <class T, long L> void h() = delete;
template <unsigned long long N> [[deprecated]] void d() {}
void t() {
  void (*p)() = f<2>;
  void (*q)(int) = g<3>;
  void (*r)() = h<X<4>, 5>;
  f<2>();
  d<9>();
}
