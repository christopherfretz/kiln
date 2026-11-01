//@ expect: compile-ok
// [expr.const.cast]/3: a prvalue of class type is converted to an xvalue
// by `const_cast<T&&>` through temporary materialization ([conv.rval]).
// This is the shape of std::indirectly_writable's
// `const_cast<const iter_reference_t<Out>&&>(*o) = t` (proxy iterators
// returning a class prvalue from operator*, libc++ test_iterators.h).
struct P {
  int *p;
  constexpr const P &operator=(int v) const {
    *p = v;
    return *this;
  }
};
constexpr P mk(int *p) { return {p}; }

template <class T, class U>
concept const_assignable = requires(T (&f)(int *), U u) {
  const_cast<const T &&>(f(nullptr)) = u;
};
static_assert(const_assignable<P, int>);
static_assert(!const_assignable<P, int *>);

constexpr int g() {
  int x = 0;
  const_cast<const P &&>(mk(&x)) = 7;
  P &&r = const_cast<P &&>(mk(&x));
  r = 8;
  return x;
}
static_assert(g() == 8);

struct D {
  int v;
  ~D() {}
};
D mkd();
int h() { return const_cast<const D &&>(mkd()).v + ((const D &&)mkd()).v; }
