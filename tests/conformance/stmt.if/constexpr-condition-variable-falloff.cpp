// [stmt.if]/2: the condition of `if constexpr` is a constant expression
// even when it declares a condition variable, so -Wreturn-type knows
// which branch an instantiation kept (the other is not instantiated) and
// sees no path off the end (a units library's `if constexpr (constexpr
// auto r = try_extract(...))`; WP-792).
//@ args: -Wreturn-type
struct opt {
  bool ok;
  constexpr explicit operator bool() const { return ok; }
};

template <int N> constexpr int f() {
  if constexpr (constexpr auto o = opt{N > 2}) return 1;
  else return 2;
}

template <int N> constexpr int g(int x) {
  if constexpr (constexpr bool o = N > 2) {
    if (x) return 1;
    return 3;
  } else if constexpr (constexpr int r = N; r == 1) {
    return 2;
  } else
    return 4;
}

int use() { return f<1>() + f<3>() + g<0>(1) + g<1>(1) + g<3>(1); }

int h(int x) {
  if (int y = x) return y;
} // expected-warning {{non-void function does not return a value in all control paths}}
