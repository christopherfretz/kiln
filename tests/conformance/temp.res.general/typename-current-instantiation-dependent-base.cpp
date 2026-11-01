// [temp.dep.type]/6, [temp.res.general]/5: `typename D::X`, for the current
// instantiation `D` with a dependent base, names a member of an unknown
// specialization: it is looked up in the instantiation.
//@ expect: run
//@ stdout: 5 7
#include <cstdio>
template <class T> struct B { typedef T X; struct In { int v = 7; }; };
template <class T> struct D : B<T> {
  int f() { typename D::X x = 5; return x; }
  int g() { typename D<T>::In i; return i.v; }
};
int main() {
  D<int> d;
  std::printf("%d %d\n", d.f(), d.g());
}
