// [namespace.udir]/2-4, [namespace.qual]/2, [namespace.def.general]/7: a
// using-directive at namespace scope belongs to that namespace and outlives
// its body; it is transitive, nominated names appear in the nearest
// enclosing namespace containing both the directive and the nominated
// namespace, qualified lookup follows a namespace's directives only when
// its own members do not declare the name, and a directive inside an
// inline namespace is seen from the enclosing namespace.
//@ strict-diags
namespace A { constexpr int x = 1; constexpr int h(int) { return 1; } }
namespace B { using namespace A; constexpr int y = 2; }
static_assert(B::x == 1 && B::y == 2);
namespace C { using namespace B; }
static_assert(C::x == 1 && C::y == 2);

// [namespace.qual]/2: B2's own `x` hides A's.
namespace B2 { using namespace A; constexpr int x = 3; }
static_assert(B2::x == 3);

// Transitive through a nested, already-closed namespace.
namespace N {
  namespace D { constexpr int f(int) { return 4; } }
  namespace E { using namespace D; }
}
constexpr int t1() { using namespace N::E; return f(0); }
static_assert(t1() == 4);

// Through nested inline namespaces, and a directive inside an inline
// namespace is seen from its enclosing namespace.
namespace P {
  namespace Q { inline namespace R { constexpr int g(int) { return 5; } } }
  inline namespace I { using namespace Q::R; }
}
static_assert(P::g(0) == 5);
constexpr int t2() { using namespace P; return g(0); }
static_assert(t2() == 5);

// The nominated names join the enclosing namespace's names: overloads
// from both are candidates ([namespace.udir]/2, [basic.lookup.unqual]/2).
namespace S {
  namespace T { constexpr int k(long) { return 6; } }
  constexpr int k(int) { return 7; }
  namespace U {
    using namespace T;
    constexpr int a = k(0L);
    constexpr int b = k(0);
  }
}
static_assert(S::U::a == 6 && S::U::b == 7);

// A directive at file scope stays in effect after a namespace closes.
using namespace C;
static_assert(x == 1 && y == 2);
