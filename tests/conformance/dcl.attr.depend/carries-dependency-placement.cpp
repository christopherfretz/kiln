// [dcl.attr.depend]/1-2 (C++23; WP-954): carries_dependency appertains to
// a function declaration or lambda or to a parameter of one; on a
// parameter of any other function declarator (a pointer to function, a
// typedef, an alias) it is clang's `only allowed on parameter in a
// function declaration or lambda`. If one declaration of a function (or
// of its parameter) specifies it, the first declaration does: a later
// one that adds it is an error noted at the first declaration (or its
// parameter). The C++26 removal is carries-dependency-removed-cxx26.cpp.
//@ std: c++23
//@ expect: compile-fail
//@ strict-diags
//@ diag: 23:14 error "'[[carries_dependency]]' attribute only allowed on parameter in a function declaration or lambda"
//@ diag: 24:19 error "'[[carries_dependency]]' attribute only allowed on parameter in a function declaration or lambda"
//@ diag: 25:19 error "'[[carries_dependency]]' attribute only allowed on parameter in a function declaration or lambda"
//@ diag: 26:48 error "'carries_dependency' attribute cannot be applied to types"
//@ diag: 27:3 error "'carries_dependency' attribute only applies to parameters, Objective-C methods, and functions"
//@ diag: 28:10 error "'carries_dependency' attribute only applies to parameters, Objective-C methods, and functions"
//@ diag: 30:15 error "parameter declared '[[carries_dependency]]' after its first declaration"
//@ diag: 29:18 note "declaration missing '[[carries_dependency]]' attribute is here"
//@ diag: 39:13 error "parameter declared '[[carries_dependency]]' after its first declaration"
//@ diag: 36:15 note "declaration missing '[[carries_dependency]]' attribute is here"
[[carries_dependency]] int *f1([[carries_dependency]] int *p);
void (*fp)([[carries_dependency]] int *);
typedef void td([[carries_dependency]] int *);
using al = void([[carries_dependency]] int *);
auto lam = []([[carries_dependency]] int *p) [[carries_dependency]] { return p; };
[[carries_dependency]] int v;
struct [[carries_dependency]] S {};
void g(int, int *p);
void g(int, [[carries_dependency]] int *p);
[[carries_dependency]] void h(int *);
[[carries_dependency]] void h(int *);
void h(int *);
struct C {
  [[carries_dependency]] void m();
  void n(int *);
};
[[carries_dependency]] void C::m() {}
void C::n([[carries_dependency]] int *) {}
