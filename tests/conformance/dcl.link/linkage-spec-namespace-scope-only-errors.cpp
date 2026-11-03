// [dcl.link]/4: a linkage-specification shall inhabit a namespace scope.
// At block scope `extern "C"` and `extern "C++"` are a storage class
// followed by a string literal: "expected unqualified-id" at the literal,
// no "type specifier required", and a braced group ends the declaration
// (clang's `SkipMalformedDecl`). In a class it is "expected member name
// or ';' after declaration specifiers".
//@ expect: compile-fail
//@ strict-diags
//@ diag: 17:10 error "expected unqualified-id"
//@ diag: 19:10 error "expected unqualified-id"
//@ diag: 20:10 error "expected unqualified-id"
//@ diag: 21:10 error "expected unqualified-id"
//@ diag: 22:10 error "expected unqualified-id"
//@ diag: 26:10 error "expected member name or ';' after declaration specifiers"
//@ diag: 27:10 error "expected member name or ';' after declaration specifiers"
void blk() {
  extern "C" void r();
  int a = 1;
  extern "C++" { void q(); }
  extern "C" int x, y;
  static "C" int w;
  extern "C";
  a = 2;
}
struct S {
  extern "C" void z();
  extern "C++" { void m(); }
  static const 5;
  int ok;
};
int use(S s) { return s.ok; }
