// [temp.local]/1-2, [class.qual]/2, [class.member.lookup]: the
// injected-class-name of a base, reached through the derived class, is a
// nested-name-specifier component: `D::B::m` names the member of the base
// specialization `B<long>`, not of the template's pattern. It works for a
// non-dependent base of a class template (`V2::Base::`) and, after
// instantiation, through a dependent base (`W::Base::` with `W : T`),
// in expressions, types, using-declarations and template arguments
// (WP-714).
//@ expect: run
//@ strict-diags
//@ stdout: 80 8 10 10 1
//@ stdout: 20 2 40 2
//@ stdout: 81 5 64 7
#include <cstdio>

template <class T> struct Base {
  static int s;
  typedef T U;
  struct In { static int z; };
  int v = 1;
  Base() = default;
  Base(int x) : v(x) {}
  static int f() { return sizeof(T); }
  static int f(int k) { return k; }
  template <class X> struct TT { static const int q = 7; };
};
template <class T> int Base<T>::s = sizeof(T) * 10;
template <class T> int Base<T>::In::z = 9;

template <int N> struct Y { static const int n = N; };

struct D : Base<long> {
  using D::Base::Base;
  int w = D::Base::s;
  int g() { D::Base::In::z++; return D::Base::f() + D::Base::v; }
};

template <class T> struct V2 : Base<short> {
  int g() {
    typename V2::Base::U u = 0;
    return V2::Base::s + V2::Base::f() + int(sizeof(u));
  }
};

template <class T> struct W : T {
  W(int x) : T(x) {}
  int g() { W::Base::s += 1; return W::Base::f(5); }
  int h() { return this->W::Base::v + Y<sizeof(typename W::Base::U)>::n; }
};

int main() {
  D d(10);
  int g = d.g() - 10;
  std::printf("%d %d %d %d %d\n", d.w, g, D::Base::In::z, d.D::Base::v,
              int(sizeof(D::Base::In)));
  V2<double> v;
  std::printf("%d %d %d %d\n", v.g() - 4, int(sizeof(V2<int>::Base::U)),
              Base<int>::s, Y<sizeof(D::Base::TT<int>)>::n + 1);
  W<D> w(3);
  w.g();
  int s = D::Base::s;
  int f = w.g();
  std::printf("%d %d %d %d\n", s, f, w.h() + 53, D::Base::TT<char>::q);
}
