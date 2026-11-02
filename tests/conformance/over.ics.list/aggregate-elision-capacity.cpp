// [over.ics.list]/8, [dcl.init.aggr]/16: whether a braced-init-list
// argument fits an aggregate parameter counts what brace elision reaches.
// Each base and member takes at least one clause: an empty class member,
// an empty-class array member (libc++'s `std::array<T, 0>`), an empty base
// and a non-aggregate class member all take `{}` or one expression; a
// subaggregate takes as many clauses as it has elements. (libc++
// alg.remove/ranges.remove_if.pass.cpp: `test<I, S, 1, 0>({.input = {1},
// .expected = {}, .cutoff = 2})` with a `std::array<int, 0>` member.)
//@ expect: compile-ok
struct E {};
struct A0 { alignas(int) E elems[4]; };
struct A1 { int e[1]; };
struct D1 { A1 input; A0 expected; int cutoff; };
constexpr int f1(D1 d) { return d.cutoff; }
static_assert(f1({{1}, {}, 2}) == 2);
static_assert(f1({.input = {1}, .expected = {}, .cutoff = 2}) == 2);

struct D2 { int i; E x; int c; };
constexpr int f2(D2 d) { return d.i + d.c; }
static_assert(f2({1, {}, 2}) == 3);
static_assert(f2({1, E{}, 2}) == 3);

struct B { int x; };
struct D3 : B { int y; };
constexpr int f3(D3 d) { return d.x * 10 + d.y; }
static_assert(f3({1, 2}) == 12);
static_assert(f3({{1}, 2}) == 12);

struct D4 : E { int y; };
constexpr int f4(D4 d) { return d.y; }
static_assert(f4({{}, 2}) == 2);

struct C { constexpr C() {} constexpr C(int) {} };
struct D5 { C c; int y; };
constexpr int f5(D5 d) { return d.y; }
static_assert(f5({1, 2}) == 2);
static_assert(f5({{}, 2}) == 2);

// Brace elision into a nested aggregate: each scalar leaf is one clause.
struct P { int a, b; };
struct D6 { P p; E e; int c; };
constexpr int f6(D6 d) { return d.p.a + d.p.b + d.c; }
static_assert(f6({1, 2, {}, 3}) == 6);

// Overload resolution: the aggregate now competes with a scalar overload.
constexpr char g(D2) { return 'a'; }
constexpr long g(long) { return 0; }
static_assert(g({1, {}, 2}) == 'a');

template <int N, int M> struct Data { A1 in; E out[M == 0 ? 1 : M]; int k; };
template <int N, int M> constexpr int t(Data<N, M> d) { return d.k; }
static_assert(t<1, 0>({.in = {1}, .out = {}, .k = 7}) == 7);
