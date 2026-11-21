// [basic.link]/15.1: the body of a function that is not inline is ignored,
// and a local class or closure type of that body is part of it whatever
// its members' specifiers: a constexpr or inline member function's body,
// a base class, a data member's type and a parameter type there name no
// TU-local entity in an exposure, nor does a lambda called in the body.
// So is a class of a function with a deduced return type that the type
// does not name, and the body of a member function that is not inline
// ([class.mfct]/1) of a class it names. GCC 16.1 and clang 22 accept each
// too (without a warning). The classes a deduced return type names are in
// exposure-local-class-escape.cpp; an importer of these functions is
// module.interface/exposure-local-class-importer-run.cpp.
//@ expect: compile-ok
//@ std: c++20
//@ strict-diags
export module M;
static int hidden;
static int helper(int x) { return x; }
namespace { struct S { int v; }; }

export int constexpr_member() { struct L { constexpr int *g() { return &hidden; } }; return *L().g(); }
export int constexpr_call() { struct L { constexpr int g(int x) { return helper(x); } }; return L().g(1); }
export int inline_member() { struct L { inline int g(int x) { return helper(x); } }; return L().g(1); }
export int declarations() { struct L : S { S s; int g(S); }; return 0; }
export int called_lambda(int x) { return [=] { return helper(x); }(); }
export auto not_returned() { struct L { S s; int v = 2; }; return L().v; }
export auto member_body() { struct L { int g(int x) { return helper(x); } }; return L(); }
