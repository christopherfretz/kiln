// [basic.link]/15.1, 15.2, 16.5 (WP-1856): the deduced types that stay
// valid next to exposure-deduced-type.cpp. A local class of a
// specialization that is not TU-local (its local class's member is not
// inline, so it does not expose `helper`); a variable or function that is
// TU-local itself; a block-scope variable in a function that is not
// inline; a declaration in the private module fragment (/18 excludes it).
// clang 22 accepts all of it; GCC 16.1 rejects `in_private`.
//@ expect: compile-ok
//@ std: c++20
//@ skip(gcc): GCC 16.1 rejects in_private in the private module fragment
//@ strict-diags
export module M;
static int helper() { return 1; }
namespace { struct S { int v; }; }
template <class T> auto ok(T) { struct L { int g() { return helper(); } }; return L(); }
export auto v4 = ok(1);
static auto tu_local = S();
namespace { auto tu_local_f() { return S(); } }
export int not_inline() { auto s = S(); return s.v + helper(); }
export auto made = [] { return 1; }();
module :private;
auto in_private = S();
