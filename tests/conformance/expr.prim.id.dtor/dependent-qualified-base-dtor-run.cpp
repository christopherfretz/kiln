// [expr.prim.id.dtor]/1, [basic.lookup.qual]/4, [class.dtor]/14: in a
// template, `this->B<T>::~B()` and `this->template B<T>::~B()` name the
// destructor of the dependent base `B<T>` and call it on the base
// subobject (non-virtually) once the template is instantiated. clang 22
// rejects both ("destructor type 'B<int>' ... does not match the type
// 'D<int>'"); GCC 16.1 accepts them.
//@ expect: run
//@ oracle: gcc
//@ xfail(clang): clang 22 compares the destructor's class with the object's, not the qualifier's
//@ stdout: ~B 1;~B 2;
#include <cstdio>
#include <new>
template <class T> struct B {
  T n = 0;
  ~B() { std::printf("~B %d;", int(n)); }
};
template <class T> struct D : B<T> {
  void f() { this->n = 1; this->B<T>::~B(); }
  void g() { this->n = 2; this->template B<T>::~B(); }
};
int main() {
  alignas(D<int>) unsigned char buf[sizeof(D<int>)];
  D<int> *d = ::new (buf) D<int>;
  d->f();
  d = ::new (buf) D<int>;
  d->g();
  std::printf("\n");
}
