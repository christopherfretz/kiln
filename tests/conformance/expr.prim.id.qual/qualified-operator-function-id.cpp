//@ expect: run
//@ strict-diags
//@ stdout: 2 20 2 3 1 7 5 9 4
// [expr.prim.id.qual]/1, [over.oper.general]/1, [class.conv.fct]/1: the
// unqualified-id of a qualified-id may be an operator-function-id or a
// conversion-function-id, after any nested-name-specifier (a
// template-id `B<float>::`, a dependent `B<T>::`, a namespace, `::`).
// libc++ <__functional/hash.h>: `__scalar_hash<float>::operator()(__v)`.
#include <new>
#include <stdio.h>

template <class T> struct B {
  int operator()(T v) const { return (int)v + 1; }
  bool operator==(const B &) const { return true; }
  operator T() const { return T(3); }
};
struct D : B<float> {
  int operator()(double v) const { return B<float>::operator()(v) * 10; }
  int g(float v) const { return this->B<float>::operator()(v); }
  bool h() const { return B<float>::operator==(*this); }
  int c() const { return (int)B<float>::operator float(); }
};
template <class T> struct E : B<T> {
  int k(T v) const { return B<T>::operator()(v); }
  T conv() const { return B<T>::operator T(); }
};

struct S { int operator+(int x) const { return x + 2; } };
int (S::*pm)(int) const = &S::operator+;

namespace N {
struct P { int v; };
int operator*(P a, P b) { return a.v * b.v; }
}  // namespace N

int main() {
  D d;
  E<int> e;
  S s;
  void *raw = ::operator new(sizeof(int));
  ::operator delete(raw);
  printf("%d %d %d %d %d %d %d %d %d\n", d.g(1), d(1.0), e.k(1), d.c(),
         (int)d.h(), (s.*pm)(5), S().S::operator+(3), N::operator*({3}, {3}),
         e.conv() + 1);
}
