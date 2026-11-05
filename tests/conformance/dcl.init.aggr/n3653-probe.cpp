// [dcl.init.aggr]/5 (C++14, N3653): a class with default member
// initializers is still an aggregate; omitted members use them.
// WP-646 feature probe.
//@ expect: compile-ok
//@ std: c++14
//@ strict-diags
struct A { int x; int y = 42; int z = x + 1; };
constexpr A a{1};
static_assert(a.x == 1 && a.y == 42 && a.z == 2, "defaults used");
constexpr A b{1, 2, 3};
static_assert(b.y == 2 && b.z == 3, "explicit values");
struct B { A inner; int w = inner.y * 2; };
constexpr B c{{5}};
static_assert(c.w == 84, "nested aggregate");
static_assert(__is_aggregate(A), "still an aggregate");
