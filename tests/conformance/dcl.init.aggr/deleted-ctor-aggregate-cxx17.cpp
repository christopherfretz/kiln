// [dcl.init.aggr]/1 (C++11-17, with DR1518): only a user-provided or
// explicit constructor makes a class a non-aggregate, so one whose
// constructors are all deleted or defaulted on their first declaration
// is still an aggregate and is aggregate-initialized; C++11/14 aggregates
// have no base classes (P0017R1 allowed them in C++17). The C++20 rule is
// p1008-probe.cpp. WP-1090.
//@ std: c++17
//@ expect: run
//@ strict-diags
//@ stdout: 1 2 3 4 5
extern "C" int printf(const char *, ...);
struct B { B(...) = delete; int x; };
struct D { D(int = 0) = delete; int x; };
struct E { E() = delete; int x; };
struct F { F() = default; int x; };
struct H { template <class... X> H(X...) = delete; int x; };
struct X { explicit X() = default; int x; };
struct P { P() {} int x; };
struct Base { int b; };
struct Derived : Base { int d; };
static_assert(__is_aggregate(B) && __is_aggregate(D) && __is_aggregate(E), "");
static_assert(__is_aggregate(F) && __is_aggregate(H), "");
static_assert(!__is_aggregate(X) && !__is_aggregate(P), "");
static_assert(__is_aggregate(Derived), "");
// Literal through being an aggregate ([basic.types.general]/10.5.2).
static_assert(__is_literal_type(B) && __is_literal_type(D) && __is_literal_type(H), "");
constexpr B b{1};
constexpr D d{2};
constexpr E e{3};
constexpr F f{4};
constexpr H h{5};
static_assert(b.x == 1 && d.x == 2 && e.x == 3 && f.x == 4 && h.x == 5, "");
int main() { printf("%d %d %d %d %d\n", b.x, d.x, e.x, f.x, h.x); }
