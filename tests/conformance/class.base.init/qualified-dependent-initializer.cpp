// [class.base.init]/2, [temp.dep.type]/6: a qualified mem-initializer-id
// whose qualifier can name an unknown specialization (`T::Self`, or the
// current instantiation `D::B` with a dependent base) names a type looked
// up when the constructor is instantiated.
//@ expect: run
//@ stdout: 1 20 3 4 50 6
#include <cstdio>
template <class T> struct B { int v; B(int x) : v(x) {} };
struct Q { int q; Q(int x) : q(x * 10) {} using Self = Q; };
template <class T> struct D6 : B<T> { D6() : D6::B(1) {} };
template <class T> struct D7 : T { D7() : T::Self(2) {} };
namespace n { template <class T> struct D8 : B<T> { D8(); }; }
template <class T> n::D8<T>::D8() : n::D8<T>::B(3) {}
template <class T> struct D9 : B<T>, Q { D9() : D9::B(4), D9::Q(5) {} };
template <class T> struct D10 : B<T> { template <class U> D10(U u) : D10<T>::B(u) {} };
int main() {
  D6<int> a; D7<Q> b; n::D8<char> c; D9<long> d; D10<int> e(6);
  std::printf("%d %d %d %d %d %d\n", a.v, b.q, c.v, d.v, d.q, e.v);
}
