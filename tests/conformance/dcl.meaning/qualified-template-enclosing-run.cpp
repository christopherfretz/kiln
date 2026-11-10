// [dcl.meaning.general]/2, [namespace.memdef]/2, [temp.expl.spec]/3: a
// qualified template declaration in a namespace enclosing its qualifier's
// is valid: an out-of-line function template, member template, variable
// template, class template and nested class template definition, an
// explicit specialization (with and without a template-id) and a member
// specialization of a class template member, from the namespace itself,
// an enclosing namespace or the global namespace (WP-1174).
//@ expect: run
//@ std: c++26
namespace A {
template <class T> int f();
template <class T> struct S {
  int g();
  template <class U> int h();
  static int v;
  struct N;
  template <class U> struct M;
};
template <class T> extern int vt;
template <class T> struct C;
namespace I {
template <class T> int k(T);
template <class T> struct D;
} // namespace I
// From the enclosing namespace.
template <class T> int I::k(T) { return 1; }
template <class T> struct I::D { int x = 2; };
template <> int I::k(int) { return 3; }
template <> struct I::D<int> { int x = 4; };
} // namespace A
template <class T> int A::f() { return 5; }
template <class T> int A::S<T>::g() { return 6; }
template <class T> template <class U> int A::S<T>::h() { return 7; }
template <class T> int A::S<T>::v = 8;
template <class T> struct A::S<T>::N { int x = 9; };
template <class T> template <class U> struct A::S<T>::M { int x = 10; };
template <class T> int A::vt = 11;
template <class T> struct A::C { int x = 12; };
// Explicit specializations: template-id and deduced, and member
// specializations of a class template's members.
template <> int A::f<char>() { return 13; }
template <> int A::I::k(long) { return 14; }
template <> int A::S<char>::g() { return 15; }
template <> template <> int A::S<char>::h<int>() { return 16; }
template <> int A::S<char>::v = 17;
template <> struct A::S<char>::N { int x = 18; };
template <> int A::vt<char> = 19;
template <> struct A::C<char> { int x = 20; };
template <class T> struct A::C<T *> { int x = 21; };

int main() {
  int r = 0;
  r |= A::I::k(1.0) != 1;
  r |= A::I::D<long>{}.x != 2;
  r |= A::I::k(1) != 3;
  r |= A::I::D<int>{}.x != 4;
  r |= A::f<int>() != 5;
  r |= A::S<int>{}.g() != 6;
  r |= A::S<int>{}.h<int>() != 7;
  r |= A::S<int>::v != 8;
  r |= A::S<int>::N{}.x != 9;
  r |= A::S<int>::M<int>{}.x != 10;
  r |= A::vt<int> != 11;
  r |= A::C<int>{}.x != 12;
  r |= A::f<char>() != 13;
  r |= A::I::k(1L) != 14;
  r |= A::S<char>{}.g() != 15;
  r |= A::S<char>{}.h<int>() != 16;
  r |= A::S<char>::v != 17;
  r |= A::S<char>::N{}.x != 18;
  r |= A::vt<char> != 19;
  r |= A::C<char>{}.x != 20;
  r |= A::C<int *>{}.x != 21;
  return r;
}
