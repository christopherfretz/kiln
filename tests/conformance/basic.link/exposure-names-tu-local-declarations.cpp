// [basic.link]/14-17: in a module interface, a declaration that names a
// TU-local entity (a type with internal linkage, a function with internal
// linkage, or a class template specialization with a TU-local type argument,
// /16.4) outside the parts /15 ignores is an exposure: a function's return
// or parameter type and its default argument, a base class, a data member's
// type, a variable's type whether exported or with module linkage. Reported
// once per declaration at its name. GCC 16.1 rejects each; clang 22 only
// warns (-WTU-local-entity-exposure); kiln follows the text and GCC.
//@ expect: compile-fail
//@ std: c++20
//@ skip(clang): clang 22 only warns
//@ strict-diags
//@ diag: 30:10 error "declaration of 'make' exposes TU-local entity 'S'"
//@ diag: 31:13 error "declaration of 'take' exposes TU-local entity 'S'"
//@ diag: 32:13 error "declaration of 'with_default' exposes TU-local entity 'helper'"
//@ diag: 33:15 error "declaration of 'Derived' exposes TU-local entity 'S'"
//@ diag: 34:15 error "declaration of 'Holder' exposes TU-local entity 'S'"
//@ diag: 35:10 error "declaration of 'var' exposes TU-local entity 'S'"
//@ diag: 36:3 error "declaration of 'module_linkage_var' exposes TU-local entity 'S'"
//@ diag: 37:14 error "declaration of 'spec_ptr' exposes TU-local entity 'W<(anonymous namespace)::S>'"
export module M;
static int helper() { return 1; }
static int target;
namespace {
struct S { int v; };
inline int local_inline() { return helper(); }
}
template <class T> struct W { T t; };

export S make();
export void take(S);
export void with_default(int = helper());
export struct Derived : S {};
export struct Holder { S m; };
export S var;
S module_linkage_var;
export W<S> *spec_ptr;
