// [basic.link]/15.1: the body of a function that is not inline is ignored,
// but not its deduced return type. A local class or closure type of such
// a body is part of it, except one the deduced return type names (the
// class itself, a pointer to it, a template argument, a class holding it
// as a member or base, a class nested in it): then the class is reached
// from an exposure and all of its declarations are checked as in an
// inline function. Naming a TU-local entity in them is an exposure: a
// data member's type and a base class (the error at the class), the body
// of a member function that is inline (`constexpr` or `inline`), a
// default member initializer, a parameter type, a default argument, and
// the body of a returned lambda (the error at the function). GCC 16.1
// rejects each (and the lambda of `in_lambda` too); clang 22 accepts. The
// classes the deduced type does not name are in
// exposure-local-class-not-inline-accepted.cpp.
//@ expect: compile-fail
//@ std: c++20
//@ skip(clang): clang 22 does not check a deduced return type's local class
//@ strict-diags
//@ diag: 40:31 error "declaration of 'L' exposes TU-local entity 'S'"
//@ diag: 41:29 error "declaration of 'L' exposes TU-local entity 'S'"
//@ diag: 42:58 error "declaration of 'g' exposes TU-local entity 'hidden'"
//@ diag: 43:51 error "declaration of 'g' exposes TU-local entity 'helper'"
//@ diag: 44:32 error "declaration of 'closure' exposes TU-local entity 'helper'"
//@ diag: 45:32 error "declaration of 'L' exposes TU-local entity 'S'"
//@ diag: 46:30 error "declaration of 'L' exposes TU-local entity 'S'"
//@ diag: 47:29 error "declaration of 'I' exposes TU-local entity 'S'"
//@ diag: 48:42 error "declaration of 'N' exposes TU-local entity 'S'"
//@ diag: 49:35 error "declaration of 'L' exposes TU-local entity 'helper'"
//@ diag: 50:42 error "declaration of 'g' exposes TU-local entity 'S'"
//@ diag: 51:44 error "declaration of 'g' exposes TU-local entity 'helper'"
//@ diag: 52:46 error "declaration of 'L' exposes TU-local entity 'S'"
export module M;
static int hidden;
static int helper() { return 1; }
namespace {
struct S { int v; };
}
template <class T> struct Box { T t; };

export auto member() { struct L { S s; }; return L(); }
export auto base() { struct L : S {}; return L(); }
export auto constexpr_body() { struct L { constexpr int *g() const { return &hidden; } }; return L(); }
export auto inline_body() { struct L { inline int g() const { return helper(); } }; return L(); }
export auto closure() { return [] { return helper(); }; }
export auto pointer() { struct L { S s; }; static L l; return &l; }
export auto boxed() { struct L { S s; }; return Box<L>(); }
export auto held() { struct I { S s; }; struct L { I i; }; return L(); }
export auto nested() { struct L { struct N { S s; }; N n; }; return L(); }
export auto field_init() { struct L { int m = helper(); }; return L(); }
export auto parameter() { struct L { int g(S); }; return L(); }
export auto default_arg() { struct L { int g(int = helper()); }; return L(); }
export auto in_lambda() { return [] { struct L { S s; }; return L(); }(); }
