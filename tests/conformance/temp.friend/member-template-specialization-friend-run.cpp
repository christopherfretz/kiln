// [temp.friend]/1.2, [temp.deduct.decl]: `friend void A::f(int);`, where
// `A` declares `f` only as a member function template, names the
// specialization `A::f<int>` that the declared type deduces; only it is
// befriended. kiln rejected it ("friend declaration of 'f' does not match
// any declaration in 'A'"; found running GCC's C++ testsuite). Also with
// explicit arguments (`A::f<>(char)`, `A::f<short>(short)`), for a static
// member template, for a member template of a class template
// specialization, and where a non-template member of that type exists it
// is the friend (the -denied sibling shows the specializations it leaves
// out).
//@ expect: run
//@ stdout: 1 2 3 4 5 6
extern "C" int printf(const char *, ...);
struct A {
  template <class T> int f(T);
  template <class T> static int s(T);
  int g(int);
  template <class T> int g(T);
};
template <class T> struct C { template <class U> int f(U); };
class B {
  friend int A::f(int);
  friend int A::f<>(char);
  friend int A::f<short>(short);
  friend int A::s(int);
  friend int A::g(int);
  friend int C<int>::f(int);
  static int x;
};
int B::x = 1;
template <> int A::f(int) { return B::x; }
template <> int A::f(char) { return B::x + 1; }
template <> int A::f(short) { return B::x + 2; }
template <> int A::s(int) { return B::x + 3; }
int A::g(int) { return B::x + 4; }
template <> template <> int C<int>::f(int) { return B::x + 5; }
int main() {
  A a;
  C<int> c;
  printf("%d %d %d %d %d %d\n", a.f(0), a.f('c'), a.f(short(0)), A::s(0), a.g(0), c.f(0));
}
