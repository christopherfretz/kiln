// [dcl.init.aggr]/1-2 (C++17, P0017R1): aggregates with public non-virtual
// base classes; the bases are initialized first. WP-646 feature probe.
//@ expect: run
//@ std: c++17
//@ strict-diags
//@ stdout: 1 2 3 0 0
extern "C" int printf(const char*, ...);
struct B1 { int a; };
struct B2 { int b; };
struct D : B1, B2 { int c; };
struct E : D {};
static_assert(__is_aggregate(D) && __is_aggregate(E));
constexpr D d{{1}, {2}, 3};
static_assert(d.a == 1 && d.b == 2 && d.c == 3);
constexpr D brace_elided{1, 2, 3};
static_assert(brace_elided.c == 3);
int main() {
  E e{};
  printf("%d %d %d %d %d\n", d.a, d.b, d.c, e.a, e.c);
}
