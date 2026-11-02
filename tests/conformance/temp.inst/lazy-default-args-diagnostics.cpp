// [temp.inst]/12 (WP-156): only the default argument a call uses is
// instantiated, at that call; its conversion failure is reported at the
// parameter, under clang's "in instantiation of default function argument
// expression" note, with the "passing argument to parameter" note.
//@ expect: compile-fail
//@ diag: 14:12 error "no viable conversion from 'const int' to 'B'"
//@ diag: 26:3 note "in instantiation of default function argument expression for 'f<B>' required here"
//@ diag: 19:8 note "candidate constructor (the implicit copy constructor) not viable: no known conversion from 'const int' to 'const B &' for 1st argument"
//@ diag: 19:8 note "candidate constructor (the implicit move constructor) not viable: no known conversion from 'const int' to 'B &&' for 1st argument"
//@ diag: 14:12 note "passing argument to parameter 'x' here"
//@ requires: clang<22
// clang 22 no longer emits error: no viable conversion from 'const int' to 'B'; WP-355 residual
template <class T> struct A {
  void f(T x = T::value) {}
  static int g(int y = T::nope) { return y; }
  template <class U> void h(U u = T::bad) {}
  A(int z = T::value) {}
};
struct B { static const int value = 3; };
void use() {
  A<int> a(1);
  a.f(1);
  A<int>::g(2);
  a.h(0);
  A<B> b;
  b.f();
}
