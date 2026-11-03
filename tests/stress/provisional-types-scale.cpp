// [basic.types.general]/5, [expr.const] + kiln Oven type table (spec/oven.md
// "Types"): a class interned by the Oven while it has no definition keeps a
// provisional slot that is rebuilt once the class completes. 5,000
// forward-declared classes, each named by two constant evaluations, keep
// every class provisional for the rest of the TU. The Oven rebuilds
// provisional slots only when sema has completed a class since its last
// look (the completion epoch), and checks membership through a hash set,
// so each evaluation costs the same few steps however many incomplete
// classes came before it (before WP-471 every evaluation scanned every
// provisional slot: O(n^2) in the number of forward declarations).
//@ expect: compile-ok
//@ strict-diags
//@ std: c++20
//@ oven: evaluations<=10000 steps<=40000 peak_bytes<=0 allocs<=0
struct Done;
constexpr Done *pdone = nullptr;
#define X(a) struct S##a; constexpr S##a *p##a = nullptr; static_assert(!p##a);
#define D(a) X(a##0) X(a##1) X(a##2) X(a##3) X(a##4) X(a##5) X(a##6) X(a##7) X(a##8) X(a##9)
#define C(a) D(a##0) D(a##1) D(a##2) D(a##3) D(a##4) D(a##5) D(a##6) D(a##7) D(a##8) D(a##9)
#define B(a) C(a##0) C(a##1) C(a##2) C(a##3) C(a##4) C(a##5) C(a##6) C(a##7) C(a##8) C(a##9)
B(0) B(1) B(2) B(3) B(4)
// A class completed after all of them: the epoch moves once, the next
// evaluation upgrades it, and the rest stay provisional.
struct Done { int v; };
constexpr Done done{7};
static_assert(done.v == 7 && !pdone);
