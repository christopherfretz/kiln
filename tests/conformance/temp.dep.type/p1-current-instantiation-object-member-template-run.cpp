// [temp.dep.type]/1.2, [temp.names]/3.2: inside class template `B`,
// `B<N>` names the current instantiation exactly as the
// injected-class-name does, so a `<` after `x.f` / `p->f` on an object of
// type `B<N>` (or `B<N> *`) starts a template argument list when lookup
// in the current instantiation finds a member template `f`; no
// `template` keyword is needed. abseil
// absl/strings/internal/charconv_bigint.cc (`copy.DivMod<10>()`, WP-686).
// Verified against clang 19.
//@ expect: run
//@ stdout: 7 3 10 11 12
#include <cstdio>
template <int N> struct B {
  unsigned v;
  template <int D> unsigned DivMod() { unsigned r = v % D; v /= D; return r; }
  unsigned f() const;
  unsigned g() const { B<N> copy = *this; return copy.DivMod<10>(); }
  unsigned h() const { B<N> c = *this; B<N> *p = &c; return p->DivMod<N>(); }
};
template <int N> unsigned B<N>::f() const {
  B<N> copy = *this;
  return copy.DivMod<10>();
}
template <class T> struct C {
  template <class U> T as(U u) const { return T(u) + 10; }
  T k() const { C<T> c; return c.as<T>(1) + 0 * c.as<int>(0); }
};
int main() {
  B<7> a{57}, b{13};
  C<int> c;
  std::printf("%u %u %u %d %d\n", a.f(), b.g(), C<int>{}.as<int>(0), c.k(), C<int>{}.as(2));
}
