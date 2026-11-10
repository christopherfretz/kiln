// [temp.expl.spec]/3, [temp.spec.partial.general]/4: the valid placements
// of a member specialization of an implicit class template specialization
// and of a specialization of a member template: the global scope (it
// encloses N), N itself (with or without the redundant N::), the inline
// namespace that declares the class (WP-1178), and, for a member template,
// the class itself (CWG727).
//@ expect: run
//@ std: c++26
//@ exit: 0
namespace N {
template <class T> struct A {
  struct I;
  enum class E;
  enum U : int;
  int f() { return 0; }
  static int v;
  template <class X> struct C { int c = 0; };
  template <class X> int g(X) { return 0; }
  template <class X> static constexpr int w = 0;
};
template <class T> int A<T>::v = 0;
inline namespace L {
template <class T> struct B {
  struct I;
  int f() { return 0; }
};
} // namespace L
} // namespace N

// The global scope encloses N.
template <> struct N::A<int>::I { int a = 1; };
template <> enum class N::A<int>::E { x = 2 };
template <> enum N::A<int>::U : int { y = 3 };
template <> int N::A<int>::f() { return 4; }
template <> int N::A<int>::v = 5;
template <> template <class X> struct N::A<int>::C<X *> { int c = 6; };
template <> template <> struct N::A<int>::C<int> { int c = 7; };
template <> template <> int N::A<int>::g<int>(int) { return 8; }
template <> template <> constexpr int N::A<int>::w<int> = 9;

// N itself, with and without the qualifier.
namespace N {
template <> struct A<long>::I { int a = 10; };
template <> struct N::A<char>::I { int a = 11; };
template <> int A<long>::f() { return 12; }
template <> template <> struct A<long>::C<int> { int c = 13; };
} // namespace N

// The inline namespace that declares B, and N, which encloses it.
namespace N {
inline namespace L {
template <> struct B<int>::I { int b = 14; };
template <> int B<int>::f() { return 15; }
} // namespace L
template <> struct B<long>::I { int b = 16; };
} // namespace N
template <> struct N::L::B<char>::I { int b = 17; };

// In the class itself: a specialization of its own member template.
struct S {
  template <class X> struct T { int t = 0; };
  template <> struct T<int> { int t = 18; };
};

int main() {
  int r = 0;
  r |= N::A<int>::I{}.a != 1;
  r |= int(N::A<int>::E::x) != 2;
  r |= N::A<int>::y != 3;
  r |= N::A<int>{}.f() != 4;
  r |= N::A<int>::v != 5;
  r |= N::A<int>::C<char *>{}.c != 6;
  r |= N::A<int>::C<int>{}.c != 7;
  r |= N::A<int>{}.g(0) != 8;
  r |= N::A<int>::w<int> != 9;
  r |= N::A<int>::w<char> != 0;
  r |= N::A<long>::I{}.a != 10;
  r |= N::A<char>::I{}.a != 11;
  r |= N::A<long>{}.f() != 12;
  r |= N::A<long>::C<int>{}.c != 13;
  r |= N::A<long>::C<char>{}.c != 0;
  r |= N::B<int>::I{}.b != 14;
  r |= N::B<int>{}.f() != 15;
  r |= N::B<long>::I{}.b != 16;
  r |= N::B<char>::I{}.b != 17;
  r |= S::T<int>{}.t != 18;
  r |= S::T<long>{}.t != 0;
  return r;
}
