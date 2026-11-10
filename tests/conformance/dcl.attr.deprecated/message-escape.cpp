// [dcl.attr.deprecated]/1, [dcl.fct.def.delete]/1 (C++26, P2573R2),
// [dcl.attr.nodiscard]/2, [lex.charset]: an attribute's or a deleted
// function's message is printed as clang escapes a string argument
// (`pushEscapedString`, spec/diag.md §6.2): the control characters
// written raw in these literals (U+0001, U+007F; an unevaluated string
// cannot spell them as escapes, [lex.string.uneval]) print as <U+XXXX>
// (WP-1259: kiln printed them raw). clang 19 and 22 agree.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 22:3 warning "'f' is deprecated: d<U+0001>e<U+007F>"
//@ diag: 17:3 note "'f' has been explicitly marked deprecated here"
//@ diag: 23:3 error "call to deleted function 'g': r<U+0001>s"
//@ diag: 18:6 note "candidate function has been explicitly deleted"
//@ diag: 24:3 warning "ignoring return value of function declared with 'nodiscard' attribute: n<U+0001>"
//@ diag: 25:3 warning "'T' is deprecated: t<U+0001>"
//@ diag: 20:10 note "'T' has been explicitly marked deprecated here"
[[deprecated("de")]] void f();
void g() = delete("rs");
[[nodiscard("n")]] int k();
struct [[deprecated("t")]] T {};
int h() {
  f();
  g();
  k();
  T t;
  return 0;
}
