//@ expect: run
//@ stdout: dtor 1
//@ stdout: 0 1 7
// [expr.call]/5, [temp.dep.expr]/1, [expr.prim.id.dtor]/2: a call of a
// (pseudo-)destructor through a dependent object stays dependent until
// instantiation, then is a `void` call: with a class type it runs the
// destructor, with a scalar it has no effect. `noexcept` of it and
// `decltype` of it are computed per instantiation.
#include <new>
#include <stdio.h>

struct A {
  int v;
  ~A() noexcept(false) { printf("dtor %d\n", v); }
};

template <class T> int destroy(T *p) {
  using R = decltype(p->~T());
  static_assert(__is_same(R, void));
  p->~T();
  return noexcept(p->~T()) ? 1 : 0;
}

template <class T> int destroy_obj(T &t) {
  (t.~T());
  return 7;
}

int main() {
  alignas(A) unsigned char buf[sizeof(A)];
  A *a = new (buf) A{1};
  int n = destroy(a);
  int i = 5;
  int m = destroy(&i);
  long l = 3;
  int k = destroy_obj(l);
  printf("%d %d %d\n", n, m, k);
}
