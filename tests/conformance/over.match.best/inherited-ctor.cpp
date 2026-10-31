// [over.match.best]/2.7: a constructor of the derived class is better than
// an inherited constructor of a base when the parameter types match for
// every argument (the example: B b; selects B::B(), not A(int = 0)).
// [namespace.udecl]/14: a derived constructor with the same signature hides
// the inherited one outright. [over.match.funcs]/9 (C++26 numbering of the
// inherited-ctor rule): inherited candidates compete as base constructors.
//@ expect: compile-ok
struct A { constexpr A(int = 0) : v(1) {} int v; };
struct B : A { using A::A; constexpr B() : A(5) { v = 2; } };
static_assert(B().v == 2);
static_assert(B(3).v == 1);

struct C { constexpr C(int, int = 0) : v(1) {} int v; };
struct E : C { using C::C; constexpr E(int, long = 0) : C(0) { v = 2; } };
static_assert(E(1, 2L).v == 2);
static_assert(E(1, 2).v == 1);

struct F { constexpr F(int) : v(1) {} int v; };
struct G : F { using F::F; constexpr G(int) : F(0) { v = 3; } };
static_assert(G(1).v == 3);

template <class T> struct H : T { using T::T; constexpr H() : T(0) { this->v = 4; } };
static_assert(H<A>().v == 4);
static_assert(H<A>(7).v == 1);
