// [class.dtor]/4: at the end of the class definition, overload resolution
// among the prospective destructors selects the destructor; the others are
// not the destructor (constraints decide). The selected `= default` one
// is trivial ([class.dtor]/8). libc++ <__expected/expected.h>:254
// (__union_t; expected ctor.convert.copy, ctor.unexpected.copy, arrow,
// void/dtor, void/error_or, void/ctor.convert.copy, WP-276).
// Verified against clang 19.
//@ expect: compile-ok
//@ strict-diags
template <class T> union U {
  constexpr U(int v) : t(v) {}
  constexpr ~U() requires(__is_trivially_destructible(T)) = default;
  constexpr ~U() {}
  T t;
};
struct X {
  int v;
  constexpr X(int v) : v(v) {}
  constexpr ~X() {}
};
struct Y {
  int v;
  constexpr Y(int v) : v(v) {}
};
static_assert(__is_trivially_destructible(U<Y>));
static_assert(!__is_trivially_destructible(U<X>));
constexpr bool t() { U<Y> a(1); U<X> b(2); return a.t.v + b.t.v == 3; }
static_assert(t());
