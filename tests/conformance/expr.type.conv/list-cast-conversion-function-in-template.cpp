// [expr.type.conv]/2, [dcl.init.list]/3.7, [temp.inst]: `TP<10>{y}` whose
// initialization is `y`'s conversion function `operator TP<10>()` keeps
// the written type `TP<10>` when its enclosing template is instantiated:
// `T{TP<10>{y}}` converts `y` to `TP<10>` and then `TP<10>` to `T`, not
// `y` to `T` directly (a chrono `sys_seconds{sys_days{ymd}}` in a JSON
// library's time-point reader).
//@ expect: run
//@ std: c++20

template <int N> struct TP {
  constexpr TP() = default;
  constexpr explicit TP(long x) : v(x) {}
  template <int M>
    requires(M > N)
  constexpr TP(const TP<M>& o) : v(o.v * M / N) {}
  long v = 0;
};

struct Y {
  long r;
  constexpr operator TP<10>() const { return TP<10>{r}; }
};

template <class T> constexpr T g(const Y& y) { return T{TP<10>{y}}; }
static_assert(g<TP<1>>(Y{3}).v == 30);
static_assert(g<TP<10>>(Y{4}).v == 4);

constexpr TP<1> f(const Y& y) { return TP<1>{TP<10>{y}}; }
static_assert(f(Y{3}).v == 30);

// A class with a non-trivial destructor: the conversion's temporary is
// bound under the cast.
struct D {
  long v;
  constexpr explicit D(long x) : v(x) {}
  constexpr ~D() {}
};
struct E {
  long w;
  constexpr E(const D& d) : w(d.v + 1) {}
};
struct Z {
  long r;
  constexpr operator D() const { return D{r}; }
};
template <class T> constexpr long h(const Z& z) { return T{D{z}}.w; }
static_assert(h<E>(Z{7}) == 8);

int main(int argc, char**) {
  Y y{argc + 2};
  if (g<TP<1>>(y).v != 30) return 1;
  if (h<E>(Z{argc}) != 2) return 2;
  return 0;
}
