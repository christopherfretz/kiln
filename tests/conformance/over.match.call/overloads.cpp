// [over.match.call]/1-2, [over.call.func]: a call through a name that
// denotes several functions resolves among them, qualified, parenthesized
// or not; a member call adds the object argument.
//@ expect: compile-ok
template <class T, class U> constexpr bool same = __is_same(T, U);
struct T1 {}; struct T2 {}; struct T3 {};

namespace N {
T1 f(int);
T2 f(const char *);
T3 f(int, int);
}
using N::f;
static_assert(same<decltype(f(1)), T1>);
static_assert(same<decltype(f("x")), T2>);
static_assert(same<decltype(f(1, 2)), T3>);
static_assert(same<decltype(N::f(1)), T1>);
static_assert(same<decltype((f)("x")), T2>);
static_assert(same<decltype((N::f)(1, 2)), T3>);

struct S {
  T1 m(int);
  T2 m(int) const;
  T3 m(double) &&;
  static T1 st(int);
  T2 st(long);
  T1 g() { return m(0); }
};
S s;
const S cs;
static_assert(same<decltype(s.m(0)), T1>);
static_assert(same<decltype(cs.m(0)), T2>);
static_assert(same<decltype(S().m(0.0)), T3>);
static_assert(same<decltype(s.st(0)), T1>);
static_assert(same<decltype(s.st(0L)), T2>);
static_assert(same<decltype(S::st(0)), T1>);
static_assert(same<decltype((&s)->m(0)), T1>);
