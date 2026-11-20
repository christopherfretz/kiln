// [namespace.udecl]/1, [temp.dep.type]/6.3, [temp.names]/6: a member a
// using-declarator with a dependent qualifier introduces (`using
// B<T>::f;`) is a member of an unknown specialization: `D::template
// f<A>` names whatever `f` is in the instantiated base (a static or a
// non-static member function template, a variable template), and `L::m`
// in a local class of a generic lambda stays dependent until the lambda
// is called.
//@ expect: run
//@ stdout: 11 4 11 5 4 11 25 2
#include <cstdio>
template <class T> struct B {
  int m = 3;
  template <class U> static int sf(U) { return sizeof(U); }
  template <class U> static constexpr int v = sizeof(U);
  template <class U> int mf(U) { return sizeof(U) + m; }
};
template <class T> struct D : B<T> {
  using B<T>::m; using B<T>::sf; using B<T>::v; using B<T>::mf;
  int g() { return D::template sf<char>(1) + D::template v<short> + sf(1L); }
  int h() { auto l = [this](auto x) { return D::template sf<decltype(x)>(x) + m; }; return l('a'); }
  int i() { return D::template mf<char>(1); }
  int j() { auto l = [this](auto x) { return D::template mf<decltype(x)>(x); }; return l(1L); }
};
template <class T> struct O {
  template <class U> struct I : B<U> {
    using B<U>::sf; using B<U>::m;
    int g() { return I::template sf<T>(1) + I::m; }
    template <class V> int k() { auto l = [this](auto x) { return I::template sf<V>(x) + I::m; }; return l(1); }
    int n() { auto l = [this](auto) { return I::m + m; }; return l(0); }
  };
};
template <class T> struct C { int m = sizeof(T); static int s() { return 7; } template <class U> static int t() { return sizeof(U); } };
template <class T> int f() {
  auto l = [](auto x) {
    struct L : C<decltype(x)> {
      using C<decltype(x)>::m; using C<decltype(x)>::s; using C<decltype(x)>::t;
      int g() { return L::m + m + L::s() + L::template t<T>(); }
    };
    return L().g();
  };
  return l(1L);
}
int main() {
  D<int> d;
  O<long>::I<char> i;
  std::printf("%d %d %d %d %d %d %d %d\n", d.g(), d.h(), i.g(), i.k<short>(), d.i(), d.j(), f<short>(), O<int>::I<char>().n() - 4);
}
