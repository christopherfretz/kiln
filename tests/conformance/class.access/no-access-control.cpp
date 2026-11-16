// [class.access]: clang's -fno-access-control (also GCC's) turns access
// checking off: private members, constructors and destructors, private bases
// and SFINAE all behave as if everything were public. Found running GCC's C++
// testsuite (WP-1467).
//@ args: -fno-access-control
class A {
  int x = 1;
  static int s;
  void f() {}
  A(int) {}
  ~A() = default;

public:
  A() = default;
};
int A::s = 2;
class B : A {};
class P {
  int p = 3;
};
// SFINAE sees private members as accessible too.
template <class T> auto probe(T *t) -> decltype(t->p, 1) { return 1; }
template <class T> int probe(...) { return 0; }
int g() {
  A a;
  a.f();
  B b;
  A *pa = &b;
  A c(5);
  P p;
  return a.x + A::s + pa->x + probe<P>(&p) + p.p;
}
