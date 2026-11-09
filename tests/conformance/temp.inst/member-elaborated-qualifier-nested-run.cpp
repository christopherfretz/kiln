// [temp.dep.type]/6, [dcl.type.elab]/3, [temp.inst]/3: `class B<T>::In::Deep`
// in another class template names the nested tag of whichever
// specialization `B<T>` selects (primary, explicit or partial), so W<long>
// gets the explicit specialization's two-member Deep (kiln used to drop the
// member silently; WP-1054). Valid twin of
// member-elaborated-qualifier-nested.cpp. g++ 14 prints the same.
//@ expect: run
//@ oracle: none
//@ skip(clang): clang 19 and 22 crash in codegen (FindInstantiatedDecl unreachable) on a member class of a specialization named through `class B<T>::In::`
//@ stdout: 1 2 3 4
#include <cstdio>
template <class T> struct B { struct In { class Deep { public: int y = 1; }; }; };
template <> struct B<long> { struct In { class Deep { public: int y = 2; int z = 3; }; }; };
template <class T> struct B<T *> { struct In { class Deep { public: int y = 4; }; }; };
template <class T> struct W { class B<T>::In::Deep d; };
int main() {
  W<int> a; W<long> b; W<char *> c;
  static_assert(sizeof(W<long>) == 2 * sizeof(int));
  std::printf("%d %d %d %d\n", a.d.y, b.d.y, b.d.z, c.d.y);
}
