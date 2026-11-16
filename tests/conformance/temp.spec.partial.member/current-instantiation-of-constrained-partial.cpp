// [temp.dep.type]/1, [temp.spec.partial.member]/1: inside a constrained
// partial specialization, and inside an out-of-line member definition whose
// template head is equivalent to its (requires-clause included), the name
// `A<T*>` or `B<T>` is the current instantiation of that partial
// specialization, not of a sibling partial specialization with the same
// arguments nor of the primary template: `A<T*>::I1` names its member
// without `typename`, also when the arguments are the primary's own (`B<T>`).
// Found running GCC's C++ testsuite. WP-1509.
//@ expect: run
//@ strict-diags
//@ stdout: 1 2 4 5
extern "C" int printf(const char *, ...);
template <class T> concept is_int = __is_same_as(T, int);

template <class T> struct A {};
template <is_int T> struct A<T*> {
  typedef int I1;
  static const A<T*>::I1 j1 = 1;
  static int f();
};
template <is_int T> int A<T*>::f() { A<T*>::I1 i = j1; return i; }

template <class T> struct A<T*> {
  typedef int I2;
  static const A<T*>::I2 j2 = 2;
  static int f();
};
template <class T> int A<T*>::f() { A<T*>::I2 i = j2; return i; }

template <class T> struct B;
template <is_int T> struct B<T> {
  typedef int I4;
  static const B<T>::I4 j4 = 4;
  static int f();
};
template <is_int T> int B<T>::f() { B<T>::I4 i = j4; return i; }

template <class T> struct B {
  typedef int I5;
  static const B<T>::I5 j5 = 5;
  static int f();
};
template <class T> int B<T>::f() { B<T>::I5 i = j5; return i; }

int main() {
  printf("%d %d %d %d\n", A<int*>::f(), A<float*>::f(), B<int>::f(), B<float>::f());
}
