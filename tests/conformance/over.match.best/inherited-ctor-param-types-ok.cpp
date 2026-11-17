// [over.match.best.general]/2.7, the accepted side: parameters that differ
// only by a typedef or by top-level cv have the same type ([dcl.fct]/5), so
// the constructor of the derived class wins; through two levels the more
// derived class's inherited constructor wins over the base's. A constructor
// template of D loses to an inherited non-template at /2.4, before /2.7.
// [over.match.funcs.general]/9: B's copy and move constructors are not
// candidates for initializing a D from a D (D's own are used).
// Found running GCC's C++ testsuite.
//@ expect: compile-ok
using Int = int;
struct B {
  constexpr B(int) : v(1) {}
  constexpr B() : v(0) {}
  constexpr B(const B&) : v(10) {}
  constexpr B(B&&) : v(11) {}
  int v;
};
struct D : B {
  using B::B;
  constexpr D(const Int) : B(0) { v = 2; }
  constexpr D() = default;
};
static_assert(D(1L).v == 2);
static_assert(D(1).v == 2);

struct E : D { using D::D; };
static_assert(E(1L).v == 2);

struct T : B {
  using B::B;
  template <class U> constexpr T(U) : B(0) { v = 3; }
};
static_assert(T(1).v == 1);
static_assert(T(1L).v == 3);

// D's defaulted copy and move constructors copy the base subobject with
// B's: v is 10 or 11, never the inherited constructors' direct result.
constexpr D d0;
static_assert(D(d0).v == 10);
static_assert(D(static_cast<D&&>(D())).v == 11);
