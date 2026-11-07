// [temp.local]/1-2, [class.qual]/2: the injected-class-name of a base
// class template specialization, found through the derived class, names
// that specialization when no `<` follows (`B`, `D::B`, `typename D::B`)
// and the template when one does (abseil raw_hash_set_test's
// `using Base = typename StringTable::raw_hash_set;`).
//@ expect: run
//@ stdout: 4 4 4 4 8 2 5
#include <cstdio>

template <class T> struct Base {
  int v = 4;
  Base() = default;
  Base(int x) : v(x) {}
};

struct S : Base<int> {
  using B1 = typename S::Base;
  using B2 = S::Base;
  typedef Base B3;
  Base<double> other;  // a template-id still names the template
  using B1::B1;
};

struct U : Base<int> {};
using B4 = U::Base;

template <class T> struct V : Base<T> {
  using B = typename V::Base;
  using B::B;
};

int main() {
  S::B1 a;
  S::B2 b;
  S::B3 c;
  B4 d;
  S s(8);
  V<long> w(2);
  V<int>::B x(5);
  std::printf("%d %d %d %d %d %d %d\n", a.v, b.v, c.v, d.v, s.v, w.v, x.v);
}
