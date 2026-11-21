//@ expect: compile-fail
//@ strict-diags
//@ std: c23
//@ diag: 21:13 error "parentheses must be omitted if 'deprecated' attribute's argument list is empty"
//@ diag: 22:17 error "parentheses must be omitted if '__deprecated__' attribute's argument list is empty"
//@ diag: 23:12 error "parentheses must be omitted if 'nodiscard' attribute's argument list is empty"
//@ diag: 26:14 error "expected string literal as argument of 'deprecated' attribute"
//@ diag: 27:11 error "attribute 'noreturn' cannot have an argument list"
//@ diag: 28:15 error "attribute '__noreturn__' cannot have an argument list"
//@ diag: 29:15 error "attribute 'maybe_unused' cannot have an argument list"
//@ diag: 32:21 warning "ignoring return value of function declared with 'nodiscard' attribute"
//@ diag: 32:27 warning "'g1' is deprecated"
//@ diag: 25:3 note "'g1' has been explicitly marked deprecated here"
//@ diag: 34:24 error "attribute 'fallthrough' cannot have an argument list"
//@ diag: 35:28 error "attribute '__fallthrough__' cannot have an argument list"
// C23 6.7.13.2 (WP-1852): the C spellings of the attributes clang checks
// for an argument clause after parsing it, as in C++
// (dcl.attr.grammar/argument-clause-standard-attributes.cpp):
// `deprecated` and `nodiscard` take no empty one, `noreturn`,
// `maybe_unused` and `fallthrough` none; the attribute is dropped.
[[deprecated()]] void d1(void);
[[__deprecated__()]] void d2(void);
[[nodiscard()]] int n1(void);
[[__nodiscard__()]] int n2(void);
[[gnu::deprecated()]] void g1(void);
[[deprecated(zzq)]] void d3(void);
[[noreturn()]] void r1(void);
[[__noreturn__()]] void r2(void);
[[maybe_unused()]] int u1;
[[__maybe_unused__()]] int u2;
void use(int x) {
  d1(); d2(); n1(); n2(); g1();
  switch (x) {
  case 0: [[fallthrough()]];
  case 1: [[__fallthrough__()]];
  case 2: break;
  }
}
