// [temp.dep.type]/6, [basic.lookup.qual.general]/2: in a generic lambda
// of a function template, a qualifier rooted in the lambda's dependent
// object (`u.B<decltype(u)>::get()`, `u.Base::get()`, `p->In::Base::~Base()`)
// or naming a member of a dependent type (`decltype(u)::In::template Y`)
// stays dependent when the enclosing template is instantiated and is
// looked up when the lambda's call operator is. (clang 19 rejects k6.)
//@ requires: clang>=22
//@ expect: run
//@ stdout: 11 2 8 14 5 1
#include <cstdio>
template <class D> struct B { int get() const { return 5; } static int sget() { return 6; } };
struct Base0 { int get() const { return 5; } };
struct D1 : B<D1>, Base0 {
  using Base = Base0;
  struct In { using Base = Base0; template <class Q> struct Y { int v = sizeof(Q); }; };
};
template <template <class> class TT, class A> int use() { return TT<A>{}.v; }
template <class T> int k() {
  auto l = [](auto u) { return u.B<decltype(u)>::get() + u.template B<decltype(u)>::sget(); };
  return l(D1{});
}
template <class T> int k2() {
  auto l = [](auto u) { return use<decltype(u)::In::template Y, T>(); };
  return l(D1{});
}
template <class T> int k3() {
  auto l = [](auto u) { return use<T::In::template Y, decltype(u)>(); };
  return l(1.0);
}
template <class T> int k4() {
  auto l = [](auto u) { return u.Base::get() + u.Base0::get() + sizeof(T); };
  return l(D1{});
}
template <class T> int k5(T t) { return t.Base::get(); }
template <class T> int k6() {
  auto l = [](auto *p) { p->In::Base::~Base(); return 1; };
  D1 d;
  return l(&d);
}
int main() {
  std::printf("%d %d %d %d %d %d\n", k<int>(), k2<short>(), k3<D1>(), k4<int>(), k5(D1{}), k6<int>());
}
