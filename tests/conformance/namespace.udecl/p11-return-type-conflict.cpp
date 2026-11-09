// [namespace.udecl]/11 (CWG101), [basic.scope.scope]/4: a function
// declared in a namespace or block scope conflicts with a function a
// using-declarator introduces when their parameter-type-lists
// correspond, whatever the return types (clang IsOverload; WP-1042).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 22:13 error "target of using declaration conflicts with declaration already in scope"
//@ diag: 19:21 note "target of using declaration"
//@ diag: 21:7 note "conflicting declaration"
//@ diag: 26:7 error "declaration conflicts with target of using declaration already in scope"
//@ diag: 19:21 note "target of using declaration"
//@ diag: 25:13 note "using declaration"
//@ diag: 30:13 error "target of using declaration conflicts with declaration already in scope"
//@ diag: 19:33 note "target of using declaration"
//@ diag: 29:8 note "conflicting declaration"
//@ diag: 35:10 error "declaration conflicts with target of using declaration already in scope"
//@ diag: 19:48 note "target of using declaration"
//@ diag: 34:15 note "using declaration"
namespace ns { void foo(); void bar(int); void baz(int); }
namespace a {
  int foo(void);
  using ns::foo;
}
namespace b {
  using ns::foo;
  int foo(void);
}
void f() {
  long bar(int);
  using ns::bar;
}
template <class> struct C {
  void g() {
    using ns::baz;
    char baz(int);
  }
};
