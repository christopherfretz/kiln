// [temp.deduct.call]/3: a forwarding reference deduces A& for an lvalue.
template <class A, class B> struct same { static constexpr bool v = false; };
template <class A> struct same<A, A> { static constexpr bool v = true; };
template <class T> constexpr bool fwd_is(T&&, int which) {
  return which == 0 ? same<T, int&>::v : which == 1 ? same<T, int>::v : same<T, const int&>::v;
}
int i = 0;
const int ci = 0;
static_assert(fwd_is(i, 0));
static_assert(fwd_is(1, 1));
static_assert(fwd_is(ci, 2));
