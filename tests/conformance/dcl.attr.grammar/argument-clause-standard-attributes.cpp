//@ expect: compile-fail
//@ strict-diags
//@ args: -ferror-limit=0
//@ diag: 39:13 error "parentheses must be omitted if 'deprecated' attribute's argument list is empty"
//@ diag: 40:17 error "parentheses must be omitted if '__deprecated__' attribute's argument list is empty"
//@ diag: 41:12 error "parentheses must be omitted if 'nodiscard' attribute's argument list is empty"
//@ diag: 43:13 error "parentheses must be omitted if 'deprecated' attribute's argument list is empty"
//@ diag: 43:26 error "parentheses must be omitted if 'nodiscard' attribute's argument list is empty"
//@ diag: 49:14 error "expected string literal as argument of 'deprecated' attribute"
//@ diag: 50:19 error "expected string literal as argument of 'deprecated' attribute"
//@ diag: 51:13 error "expected string literal as argument of 'nodiscard' attribute"
//@ diag: 52:11 error "attribute 'noreturn' cannot have an argument list"
//@ diag: 53:15 error "attribute '__noreturn__' cannot have an argument list"
//@ diag: 54:15 error "attribute 'maybe_unused' cannot have an argument list"
//@ diag: 56:15 error "expected expression"
//@ diag: 57:12 error "use of undeclared identifier 'zzq'"
//@ diag: 59:22 error "attribute 'no_unique_address' cannot have an argument list"
//@ diag: 60:26 error "attribute '__no_unique_address__' cannot have an argument list"
//@ diag: 63:21 warning "ignoring return value of function declared with 'nodiscard' attribute"
//@ diag: 63:33 warning "'g1' is deprecated"
//@ diag: 44:9 note "'g1' has been explicitly marked deprecated here"
//@ diag: 63:39 warning "'g2' is deprecated"
//@ diag: 45:3 note "'g2' has been explicitly marked deprecated here"
//@ diag: 63:45 warning "ignoring return value of function declared with 'nodiscard' attribute: m"
//@ diag: 65:24 error "attribute 'fallthrough' cannot have an argument list"
//@ diag: 66:31 error "attribute 'fallthrough' cannot have an argument list"
//@ diag: 67:36 error "attribute '__fallthrough__' cannot have an argument list"
//@ diag: 68:28 error "attribute '__fallthrough__' cannot have an argument list"
//@ diag: 71:18 error "attribute 'likely' cannot have an argument list"
//@ diag: 71:45 error "attribute '__unlikely__' cannot have an argument list"
// [dcl.attr.grammar]/4, [dcl.attr.deprecated]/1, [dcl.attr.nodiscard]/1
// (WP-1852): a standard attribute whose argument clause is optional takes
// no empty one, and one that takes no arguments takes no clause at all,
// as clang's `ParseCXX11AttributeArgs`: the clause is parsed first (its
// own errors win, and then nothing more is said), the attribute is
// dropped. `__x__` is `x`, except for `nodiscard` and `maybe_unused`,
// which clang matches by their exact names; `gnu::` spellings follow the
// GNU attribute's rules; `[[clang::fallthrough]]` is checked too.
[[deprecated()]] void d1();
[[__deprecated__()]] void d2();
[[nodiscard()]] int n1();
[[__nodiscard__()]] int n2();
[[deprecated(), nodiscard()]] int dn();
[[using gnu: deprecated()]] void g1();
[[gnu::deprecated()]] void g2();
[[clang::warn_unused_result()]] int g3();
[[deprecated("m")]] void d3();
[[nodiscard("m")]] int n3();
[[deprecated(zzq)]] void d4();
[[deprecated("m", 1)]] void d5();
[[nodiscard(1)]] int n4();
[[noreturn()]] void r1();
[[__noreturn__()]] void r2();
[[maybe_unused()]] int u1;
[[__maybe_unused__()]] int u2;
[[noreturn(1 +)]] void r3();
[[noreturn(zzq)]] void r4();
struct S {
  [[no_unique_address()]] int a;
  [[__no_unique_address__()]] int b;
};
void use(int x) {
  d1(); d2(); n1(); n2(); dn(); g1(); g2(); n3();
  switch (x) {
  case 0: [[fallthrough()]];
  case 1: [[clang::fallthrough()]];
  case 2: [[_Clang::__fallthrough__()]];
  case 3: [[__fallthrough__()]];
  case 4: break;
  }
  if (x) [[likely()]] {} else [[__unlikely__()]] {}
}
