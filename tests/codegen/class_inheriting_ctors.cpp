// Inheriting constructors ([class.inhctor.init], [namespace.udecl]/3):
// Itanium CI1/CI2 complete/base variants, argument forwarding (by value,
// by reference, rvalue reference), default arguments, default member
// initializers, virtual bases (the base variant drops its parameters and
// takes a VTT; a non-most-derived call still evaluates and passes the
// arguments), empty members addressed by offset, and a dependent base.
extern "C" int printf(const char*, ...);
struct A { A(int x, int y = 7) : s(x + y) { printf("A %d\n", s); } int s; };
struct B : A { using A::A; int z = 3; };
struct V { V(int v) : v(v) { printf("V %d\n", v); } V() : v(-1) {} int v; };
struct M1 : virtual V { using V::V; };
struct M2 : M1 { using M1::M1; int k = 4; };
template <class T> struct Base { Base(T v) : v(v) {} T v; };
template <class T> struct D : Base<T> { using Base<T>::Base; };
struct S { S(int v) : v(v) {} S(const S& o) : v(o.v + 100) {} ~S() {} int v; };
struct T { T() {} ~T() {} };
struct F { F(S s, const int& r, int&& m) : x(s.v + r + m) {} int x; };
struct G : F { using F::F; T t; S s2{42}; };
int side() { return 3; }
struct M3 : M1 { M3() : M1(side()) {} };
struct N { N(int n) : n(n) {} int n; };
struct X : N { using N::N; };
struct Y : virtual X { using X::X; };
struct E {};
struct H { H(int) {} };
struct I : H { using H::H; E e; int q = 1; };
int main() {
  B b(1); printf("%d %d\n", b.s, b.z);
  M1 m1(5);
  M2 m2(6);
  D<long> d(9);
  int r = 2;
  G g(S(1), r, 4);
  M3 m3;
  Y y(7);
  I i(0);
  return b.s + m1.v + m2.v + int(d.v) + g.x + m3.v + y.n + i.q;
}
