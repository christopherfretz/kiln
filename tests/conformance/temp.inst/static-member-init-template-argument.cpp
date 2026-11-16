// [temp.inst]/8, [temp.arg.nontype]/1: a template argument is constant-
// evaluated even inside `decltype`, so the static data member's
// initializer is instantiated there and its error is reported with the
// instantiation note (WP-1497).
//@ expect: compile-fail
//@ std: c++20
//@ strict-diags
//@ diag: 10:60 error "type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 12:31 note "in instantiation of static data member 'A<int>::value' requested here"
template <class T> struct A { static constexpr int value = T::nonexistent; };
template <int> struct B {};
using t1 = decltype(B<A<int>::value>{});
