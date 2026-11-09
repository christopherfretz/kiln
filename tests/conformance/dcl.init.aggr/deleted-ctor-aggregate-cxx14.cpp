// [dcl.init.aggr]/1 (C++11/14, with DR1518): the C++14 form of
// deleted-ctor-aggregate-cxx17.cpp. Deleted or defaulted constructors
// keep a class an aggregate, and any base class makes it a non-aggregate
// (the base-class rule is C++17's P0017R1). WP-1090.
//@ std: c++14
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
static_assert(!__is_aggregate(Derived), "");
// Literal through being an aggregate ([basic.types.general]/10.5.2).
static_assert(__is_literal_type(B) && __is_literal_type(D) && __is_literal_type(H), "");
constexpr B b{1};
constexpr D d{2};
constexpr E e{3};
constexpr F f{4};
constexpr H h{5};
static_assert(b.x == 1 && d.x == 2 && e.x == 3 && f.x == 4 && h.x == 5, "");
int main() { printf("%d %d %d %d %d\n", b.x, d.x, e.x, f.x, h.x); }
