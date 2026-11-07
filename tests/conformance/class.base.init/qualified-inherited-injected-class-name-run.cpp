// [class.base.init]/2, [temp.local]/1-2: a mem-initializer-id may be a
// qualified name, `D::B`, that finds the injected-class-name of a base
// class template specialization through the derived class; it names that
// base, also in the definition of a constructor template.
//@ expect: run
//@ strict-diags
//@ std: c++17

template <typename D, typename K> struct Entry {
  int k;
  constexpr Entry() : k(1) {}
  constexpr explicit Entry(int v) : k(v) {}
};

struct E : Entry<E, int> {
  template <typename = void> explicit constexpr E(int v);
  E();
};
template <typename>
constexpr E::E(int v) : E::Entry(v) {}
E::E() : E::Entry() {}

namespace n {
template <typename T> struct Base {
  T t;
  explicit Base(T v) : t(v) {}
};
struct F : Base<long> {
  F();
};
}  // namespace n
n::F::F() : n::F::Base(7) {}

constexpr E ce(5);
static_assert(ce.k == 5, "");

int main() {
  E a(3), b;
  n::F f;
  if (a.k != 3 || b.k != 1) return 1;
  if (f.t != 7) return 2;
  return 0;
}
