// [basic.link]/15-17 (WP-1684): in a module interface, a default member
// initializer is part of its class's definition and a default argument
// part of its function's declaration, so naming a TU-local entity there
// is an exposure: a call of a TU-local function (also in a lambda, in a
// class template, in a local class of an inline function), a TU-local type
// written in a sizeof operand or an explicit conversion, and an odr-use of
// a TU-local constant (`&K`; /15.4 ignores only the references to it that
// are not odr-uses). A member function's default argument, parsed in the
// complete-class context, too. Reported at the class (the lambda) or the
// function, as GCC 16.1 does; clang 22 accepts every one.
//@ expect: compile-fail
//@ std: c++20
//@ skip(clang): clang 22 accepts
//@ strict-diags
//@ diag: 38:15 error "declaration of 'Nsdmi' exposes TU-local entity 'helper'"
//@ diag: 39:8 error "declaration of 'NsdmiModuleLinkage' exposes TU-local entity 'helper'"
//@ diag: 40:34 error "declaration of 'NsdmiTemplate' exposes TU-local entity 'helper'"
//@ diag: 41:37 error "declaration of 'NsdmiLambda' exposes TU-local entity 'helper'"
//@ diag: 42:15 error "declaration of 'NsdmiAddress' exposes TU-local entity 'K'"
//@ diag: 43:42 error "declaration of 'L' exposes TU-local entity 'helper'"
//@ diag: 44:42 error "declaration of 'g' exposes TU-local entity 'helper'"
//@ diag: 45:35 error "declaration of 'g' exposes TU-local entity 'helper'"
//@ diag: 46:35 error "declaration of 'g' exposes TU-local entity 'S'"
//@ diag: 47:13 error "declaration of 'default_sizeof' exposes TU-local entity 'S'"
//@ diag: 48:13 error "declaration of 'default_cast' exposes TU-local entity 'S'"
//@ diag: 49:13 error "declaration of 'default_address' exposes TU-local entity 'K'"

export module M;
static int helper() { return 1; }
static const int K = 5;
static constexpr int K2 = 3;
static int hidden;
namespace {
struct S { int v; };
}
inline void take(const int &) {}

export struct Nsdmi { int m = helper(); };
struct NsdmiModuleLinkage { int m{helper()}; };
export template <class T> struct NsdmiTemplate { int m = helper(); };
export struct NsdmiLambda { int m = [] { return helper(); }(); };
export struct NsdmiAddress { const int *p = &K; };
export inline int local_class() { struct L { int m = helper(); }; return L().m; }
export struct StaticDefault { static int g(int = helper()); };
export struct MemberDefault { int g(int = helper()); };
export struct MemberSizeof { void g(unsigned long = sizeof(S)); };
export void default_sizeof(unsigned long = sizeof(S));
export void default_cast(const void * = (S *)nullptr);
export void default_address(const int * = &K);
