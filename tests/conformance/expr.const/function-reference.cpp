// [expr.const]: a reference to a function designates the function, in a
// call, under `&`, and as the initializer of a constexpr reference; a no-op
// cast such as `static_cast<F&&>(f)` ([expr.static.cast]/4) designates the
// same function (libc++'s `std::__invoke` calls through one).
//@ expect: compile-ok
struct T { static constexpr bool eq(char a, char b) { return a == b; } };
template <class P> constexpr bool viaref(P &p) { return p('a', 'a') && !p('a', 'b'); }
constexpr bool f1() { return viaref(T::eq); }
static_assert(f1());
template <class F> constexpr bool fwd(F &&f) { return static_cast<F &&>(f)('x', 'x'); }
constexpr bool f2() { return fwd(T::eq) && fwd(*T::eq); }
static_assert(f2());
constexpr int twice(int x) { return 2 * x; }
constexpr int g() {
  int (&r)(int) = twice;
  int (*p)(int) = &r;
  return r(3) + p(4) + (*p)(1) + static_cast<int (&)(int)>(r)(0);
}
static_assert(g() == 16);
constexpr int (&gr)(int) = twice;
static_assert(gr(5) == 10);
static_assert(&gr == &twice);
