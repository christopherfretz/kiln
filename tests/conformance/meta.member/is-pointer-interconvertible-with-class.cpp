// __builtin_is_pointer_interconvertible_with_class(m) (GCC; what
// libstdc++'s std::is_pointer_interconvertible_with_class calls,
// [meta.member]/2): whether an object of class S (the class of m's type)
// is pointer-interconvertible with its member m ([basic.compound]/5): any
// member of a union; the first non-static data member of a standard-layout
// class (a member of its base when the base declares S's members), and
// through anonymous members transitively. False for null, a pointer to
// member function, or a class that is not standard-layout. Found running
// GCC's C++ testsuite (WP-1472); clang 19/22 do not have the builtin.
//@ oracle: gcc
//@ skip(clang): clang 19/22 do not have __builtin_is_pointer_interconvertible_with_class
//@ strict-diags
//@ std: c++20
#define PI(m) __builtin_is_pointer_interconvertible_with_class(m)

struct A { int a; double b; };
static_assert(PI(&A::a));
static_assert(!PI(&A::b));
static_assert(!PI((int A::*)nullptr));

struct F { void f(); int x; };
static_assert(!PI(&F::f));
static_assert(PI(&F::x));

// A base that declares the members of a standard-layout class.
struct E1 {};
struct E2 {};
struct B : E1, E2, A {};
struct C : E1 { int c; };
struct D : C, E2 {};
static_assert(PI(&B::a));                         // int A::*
static_assert(PI(static_cast<int B::*>(&B::a)));  // int B::*
static_assert(PI(static_cast<int D::*>(&D::c)));
static_assert(!PI(static_cast<double B::*>(&B::b)));

// Not standard-layout: mixed access, a virtual base (a member of the
// base, named through the base's own type, still is).
struct P { int p0; private: int p1; public: int p2; };
struct Q : P {};
struct V : virtual A { int v; };
static_assert(!PI(&P::p0));
static_assert(!PI(static_cast<int Q::*>(&Q::p0)));
static_assert(!PI(&V::v));
static_assert(PI(&V::a));

// Every member of a union, standard-layout or not.
union U { int i; double d; private: long l; public: char c; };
static_assert(PI(&U::i));
static_assert(PI(&U::d));
static_assert(PI(&U::c));

// [[no_unique_address]]: only the first member, whatever the offsets.
struct M { [[no_unique_address]] E1 e1; [[no_unique_address]] E2 e2; int i; };
static_assert(PI(&M::e1));
static_assert(!PI(&M::e2));
static_assert(!PI(&M::i));

// Anonymous unions and structs.
struct W { union { int a; long b; }; int c; };
static_assert(PI(&W::a));
static_assert(PI(&W::b));
static_assert(!PI(&W::c));
struct X { struct { int a; long b; }; int c; };
static_assert(PI(&X::a));
static_assert(!PI(&X::b));
static_assert(!PI(&X::c));
struct NSL { int x; private: int y; };
union Y { int a; struct { short b; NSL z; }; long d; };
static_assert(PI(&Y::a));
static_assert(!PI(&Y::b));  // the anonymous struct is not standard-layout
static_assert(PI(&Y::d));

// In a constant expression through a function, as libstdc++ calls it.
template <class S, class T>
constexpr bool is_pi(T S::*m) noexcept { return PI(m); }
static_assert(is_pi(&A::a));
static_assert(!is_pi(&A::b));
static_assert(is_pi<B, int>(&B::a));
constexpr double A::*null_b = nullptr;
static_assert(!is_pi(null_b));
