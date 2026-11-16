// __builtin_is_corresponding_member(m1, m2) (GCC; what libstdc++'s
// std::is_corresponding_member calls, [meta.member]/4): true when m1 and
// m2 name corresponding members of the common initial sequence
// ([class.mem.general]/23) of the standard-layout structs S1 and S2 (the
// classes of the member pointer types), false for a null value, a pointer
// to member function, a union or a class that is not standard-layout.
// Besides the text's conditions (layout-compatible types, the same
// alignment requirement, both or neither [[no_unique_address]], equal
// bit-field widths) the members must sit at the same offset, as in
// GCC 16.1. Found running GCC's C++ testsuite (WP-1472); clang 19/22 do
// not have the builtin.
//@ oracle: gcc
//@ skip(clang): clang 19/22 do not have __builtin_is_corresponding_member
//@ strict-diags
//@ std: c++20
#define CM(a, b) __builtin_is_corresponding_member(a, b)

struct A { int a; };
struct B { const int b; };
struct C { int c0; unsigned c1; A c2; int c3; long c4; };
struct D { int d0; unsigned d1; B d2; int : 0; int d3; long d4; };
// The unnamed bit-field ends the sequence: c3/d3 do not correspond.
static_assert(CM(&C::c0, &D::d0));
static_assert(CM(&C::c1, &D::d1));
static_assert(CM(&C::c2, &D::d2));
static_assert(!CM(&C::c3, &D::d3));
static_assert(!CM(&C::c0, &D::d1));
static_assert(!CM(&C::c1, &D::d0));
static_assert(CM(&A::a, &B::b));
static_assert(CM(&A::a, &A::a));

// Null values and member functions.
struct F { void f(); int x; };
static_assert(!CM((int A::*)nullptr, &A::a));
static_assert(!CM(&A::a, (const int B::*)nullptr));
static_assert(!CM(&F::f, &F::f));

// Members of a standard-layout class's base; the class of each pointer
// is its type's (a converted pointer takes the derived class).
struct E {};
struct G : A {};
struct H : E, B {};
struct Z : A, B {};   // not standard-layout
static_assert(CM(&G::a, &B::b));
static_assert(CM(static_cast<int G::*>(&G::a), &H::b));
static_assert(CM(&Z::a, &Z::b));  // int A::*, const int B::*
static_assert(!CM(static_cast<int Z::*>(&Z::a), static_cast<const int Z::*>(&Z::b)));

// Not standard-layout (mixed access), and unions.
struct P { int p0; private: int p1; public: int p2; };
union U { int u; };
static_assert(!CM(&P::p0, &P::p0));
static_assert(!CM(&U::u, &U::u));

// Bit-fields: the same width or none.
struct K1 { int a : 3; int b : 4; int c; };
struct K2 { int a : 3; int b : 5; int c; };
static_assert(CM(&K1::c, &K1::c));
static_assert(!CM(&K1::c, &K2::c));

// [[no_unique_address]]: both or neither (CWG2759).
struct N1 { int a; [[no_unique_address]] int b; int c; };
struct N2 { int a; [[no_unique_address]] int b; int c; };
struct N3 { int a; int b; int c; };
static_assert(CM(&N1::c, &N2::c));
static_assert(CM(&N1::a, &N3::a));
static_assert(!CM(&N1::c, &N3::c));

// The same alignment requirement.
struct L1 { int a; alignas(8) int b; int c; };
struct L2 { int a; int b; int c; };
static_assert(CM(&L1::a, &L2::a));
static_assert(!CM(&L1::c, &L2::c));

// Layout-compatible member types of different sizes: the sequence stops
// where the offsets part (as in GCC; the text alone would go on).
struct I4 { int v; };
struct alignas(16) J16 { int v; };
struct Q1 { char h; alignas(16) I4 m; short t; };
struct Q2 { char h; J16 m; short t; };
static_assert(CM(&Q1::m, &Q2::m));
static_assert(!CM(&Q1::t, &Q2::t));

// An anonymous union is a member of the sequence (its members are not:
// is-corresponding-member-anon.cpp).
struct V1 { int a; union { int b; long c; }; int d; };
struct V2 { int a; union { long b; int c; }; int d; };
static_assert(CM(&V1::d, &V2::d));

// In a constant expression through a function, as libstdc++ calls it.
template <class S1, class S2, class M1, class M2>
constexpr bool is_cm(M1 S1::*m1, M2 S2::*m2) noexcept { return CM(m1, m2); }
static_assert(is_cm(&C::c2, &D::d2));
static_assert(!is_cm(&C::c4, &D::d4));
static_assert(is_cm<G, H, int, const int>(&G::a, &H::b));
constexpr int A::*null_a = nullptr;
static_assert(!is_cm(null_a, &A::a));
