// [class.qual]/2, [namespace.udecl]/3: in a using-declarator, a name
// that, looked up in the class the qualifier nominates, is that class's
// injected-class-name names its constructors, also when the qualifier is
// a typedef of a class template specialization (folly's
// `relaxed_atomic<bool>`: `using base = relaxed_atomic_base<bool>;
// using base::relaxed_atomic_base;`).
//@ expect: run
//@ stdout: 1 3 5
#include <cstdio>

namespace d {
template <class T> struct rab {
  rab() = default;
  constexpr rab(T v) : x(v) {}
  T x{};
};
struct plain {
  plain(int v) : x(v) {}
  int x;
};
} // namespace d

template <class T> struct ra;
template <> struct ra<bool> : d::rab<bool> {
  using base = d::rab<bool>;
  using base::rab;
};

struct nt : d::rab<int> {
  using d::rab<int>::rab;
};

struct np : d::plain {
  using base = d::plain;
  using base::plain;
};

int main() {
  ra<bool> a(true);
  nt n(3);
  np p(5);
  std::printf("%d %d %d\n", a.x, n.x, p.x);
}
