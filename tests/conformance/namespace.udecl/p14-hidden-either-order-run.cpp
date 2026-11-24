// [namespace.udecl]/14: a member function of the derived class hides the
// base member a using-declarator names when their parameter-type-lists
// correspond, whether the member is declared before or after the
// using-declarator (WP-3145); the base's other overloads stay visible.
// The same for constructors brought in by `using B::B;`, and in a class
// template specialization. Each call must pick the derived class's own
// member, or reach the base's through the using-declarator.
//@ expect: run
//@ stdout: 3 2 4 2 5 2 | 30 20 40 20
extern "C" int printf(const char *, ...);
struct B {
  int f() { return 1; }
  int f(int) { return 2; }
};
struct D : B {
  using B::f;
  int f() { return 3; }
};
struct E : B {
  int f() { return 4; }
  using B::f;
};
template <class T> struct F : B {
  int f() { return 5; }
  using B::f;
};
struct C {
  int v;
  C(int) : v(10) {}
  C(double) : v(20) {}
};
struct G : C {
  using C::C;
  G(int) : C(0.0) { v = 30; }
};
struct H : C {
  H(int) : C(0.0) { v = 40; }
  using C::C;
};
int main() {
  D d;
  E e;
  F<int> f;
  int a = d.f();
  int b = d.f(0);
  int c = e.f();
  int g = e.f(0);
  int h = f.f();
  int i = f.f(0);
  printf("%d %d %d %d %d %d | ", a, b, c, g, h, i);
  G g1(1);
  G g2(1.0);
  H h1(1);
  H h2(1.0);
  printf("%d %d %d %d\n", g1.v, g2.v, h1.v, h2.v);
}
