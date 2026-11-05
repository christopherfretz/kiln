// [dcl.init.aggr]/1.1, [diff.cpp17.dcl.decl] (C++20, P1008R1): a class
// with a user-declared constructor, even a defaulted or deleted one, is
// not an aggregate. WP-646 feature probe.
//@ expect: compile-fail
//@ std: c++20
//@ strict-diags
//@ diag: 13:3 error "call to deleted constructor of 'D'"
//@ diag: 9:12 note "'D' has been explicitly marked deleted here"
struct D { D() = delete; int x; };
struct E { E() = default; int x; };
static_assert(!__is_aggregate(D) && !__is_aggregate(E));
E e{};
D d{};
