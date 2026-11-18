// [meta.reflection.substitute]/2, [dcl.spec.auto.general]/12 (P2996R13):
// naming a specialization of a function template whose declared return
// type contains a placeholder instantiates its definition, which deduces
// the return type, so can_substitute is true and substitute reflects the
// deduced specialization, for every kind of function template: namespace
// scope, static member, member with decltype(auto), a constrained
// placeholder, a generic lambda's call operator. Only a specialization
// with no reachable definition stays undeduced (false); once the
// definition is seen, the same substitution succeeds. The reflection's
// type, return_type_of, a splice call and extract see the deduced type.
// GCC 16.1 answers false for a namespace-scope template and does not
// deduce a member template's return type (docs/status/deviations.md,
// WP-1674).
//@ expect: compile-ok
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ xfail(gcc): GCC 16.1 does not deduce the return type of a namespace-scope or member function template specialization in substitute (WP-1674)
//@ strict-diags
#include <meta>
using namespace std::meta;
using size = decltype(sizeof 0);
template<class T> concept small = sizeof(T) <= 2;

template<class T> constexpr auto g() { return sizeof(T); }
struct X {
  template<class T> static constexpr auto f() { return T{}; }
  template<class T> decltype(auto) h(T& t) { return (t); }
};
template<class T> small auto ci() { return T{}; }
static constexpr auto lam = []<class T>() static { return sizeof(T); };

constexpr info rg = substitute(^^g, {^^int});
static_assert(can_substitute(^^g, {^^int}));
static_assert(type_of(rg) == ^^size());
static_assert(return_type_of(rg) == dealias(^^size));
static_assert([:rg:]() == sizeof(int));
static_assert(extract<size (*)()>(rg)() == sizeof(int));

static_assert(can_substitute(^^X::f, {^^long}));
static_assert(return_type_of(substitute(^^X::f, {^^long})) == ^^long);
static_assert(return_type_of(substitute(^^X::h, {^^int})) == ^^int&);
static_assert(return_type_of(substitute(^^ci, {^^short})) == ^^short);
static_assert(can_substitute(^^decltype(lam)::template operator(), {^^char}));
static_assert(return_type_of(substitute(^^decltype(lam)::template operator(), {^^char})) == dealias(^^size));

template<class T> auto later();
static_assert(!can_substitute(^^later, {^^int}));
template<class T> auto later() { return 1; }
static_assert(can_substitute(^^later, {^^int}));
static_assert(return_type_of(substitute(^^later, {^^int})) == ^^int);

int main() {}
