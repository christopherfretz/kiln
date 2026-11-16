// [expr.prim.req.compound]/1: what follows '->' is a type-constraint; a
// type (the Concepts TS form) is ill-formed. Found running GCC's C++ testsuite.
//@ expect: compile-fail
//@ strict-diags
//@ args: -std=c++20
//@ diag: 12:57 error "expected concept name with optional arguments"
//@ diag: 13:57 error "expected concept name with optional arguments"
//@ diag: 14:57 error "expected concept name with optional arguments"
template <class T, class U> concept C = true;
template <class T> concept D = true;
namespace N { template <class T> concept E = true; }
template <class T> concept X = requires(T t) { { t } -> bool; };
template <class T> concept W = requires(T t) { { t } -> T; };
template <class T> concept Z = requires(T t) { { t } -> const D auto &; };
template <class T> concept Y = requires(T t) {
  { t } -> C<int>;
  { t } -> D;
  { t } -> N::E;
  { t } noexcept -> D;
  requires D<T>;
};
static_assert(Y<int>);
