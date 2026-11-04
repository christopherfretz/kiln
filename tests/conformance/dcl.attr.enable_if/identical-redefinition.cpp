// clang's `enable_if` attribute (clang AttrDocs "enable_if", `Sema::IsOverload`;
// [basic.def.odr]/1, [class.mem.general]/5): declarations whose `enable_if`
// conditions are the same, in order, declare the same function, so a second
// definition is a redefinition and a second member declaration a
// redeclaration.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 15:5 error "redefinition of 'f'"
//@ diag: 14:5 note "previous definition is here"
//@ diag: 18:3 error "constructor cannot be redeclared"
//@ diag: 17:3 note "previous declaration is here"
//@ diag: 20:8 error "class member cannot be redeclared"
//@ diag: 19:8 note "previous declaration is here"
int f(int n) __attribute__((enable_if(n > 0, "pos"))) { return 1; }
int f(int n) __attribute__((enable_if(n > 0, "other message"))) { return 2; }
struct S {
  S(int n) __attribute__((enable_if(n == 1, "one")));
  S(int n) __attribute__((enable_if(n == 1, "one")));
  void m(int n) __attribute__((enable_if(n == 1, "one")));
  void m(int n) __attribute__((enable_if(n == 1, "one")));
};
