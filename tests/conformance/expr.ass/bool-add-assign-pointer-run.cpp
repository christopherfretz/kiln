// [expr.ass]/6: `b += p` is `b = b + p`. For a `bool b` the sum (the
// promoted `b` plus the pointer `p`, an array decayed first) converts back
// to `bool` ([conv.bool]), so the expression is valid. No other integer
// type takes a pointer back, and `b -= p` is invalid (`int - T*`).
// Boost.TypeTraits `has_plus_assign<bool, int*>` probes this.
//@ expect: run
//@ stdout: 1 1 0 1 1
extern "C" int printf(const char*, ...);

template <class T> T&& dv();
template <class L, class R, class = decltype(dv<L&>() += dv<R>())>
constexpr bool add(int) { return true; }
template <class L, class R> constexpr bool add(...) { return false; }
template <class L, class R, class = decltype(dv<L&>() -= dv<R>())>
constexpr bool sub(int) { return true; }
template <class L, class R> constexpr bool sub(...) { return false; }

static_assert(add<bool, int*>(0) && add<bool, int* const>(0) && add<bool, int (&)[2]>(0));
static_assert(!add<int, int*>(0) && !add<char, int*>(0) && !add<bool, void*>(0));
static_assert(!sub<bool, int*>(0));

int x[3];
bool f(int* p) {
  bool b = false;
  b += p;
  return b;
}

int main() {
  bool b = false;
  bool& r = (b += x + 1);
  bool c = true;
  c += x;
  printf("%d %d %d %d %d\n", f(x), b, f(nullptr), &r == &b, c);
}
