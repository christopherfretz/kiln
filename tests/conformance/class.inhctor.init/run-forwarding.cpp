// [class.inhctor.init]/1: the parameters are forwarded to the inherited
// constructor (a by-value class parameter is not copied again; references
// bind to the same objects); other members are initialized in order and
// destroyed in reverse. /2 ([class.base.init]/7): a base-object
// constructor of a class whose inherited constructor lives in a virtual
// base ignores the arguments, which are still evaluated.
//@ expect: run
//@ stdout: A 7
//@ stdout: T()
//@ stdout: ~S 1
//@ stdout: 7 42
//@ stdout: V()
//@ stdout: side
//@ stdout: -1
//@ stdout: 7
//@ stdout: ~S 42
//@ stdout: ~T
extern "C" int printf(const char*, ...);
struct S { S(int v) : v(v) {} S(const S& o) : v(o.v + 100) { printf("copy\n"); } ~S() { printf("~S %d\n", v); } int v; };
struct T { T() { printf("T()\n"); } ~T() { printf("~T\n"); } };
struct A { A(S s, const int& r, int&& m) : x(s.v + r + m) { printf("A %d\n", x); } int x; };
struct B : A { using A::A; T t; S s2{42}; };
struct V { V(int v) : v(v) { printf("V %d\n", v); } V() : v(-1) { printf("V()\n"); } int v; };
struct M1 : virtual V { using V::V; };
int side() { printf("side\n"); return 3; }
struct M3 : M1 { M3() : M1(side()) {} };
struct N { N(int n) : n(n) {} int n; };
struct X : N { using N::N; };
struct Y : virtual X { using X::X; };
int main() {
  int r = 2;
  B b(S(1), r, 4); printf("%d %d\n", b.x, b.s2.v);
  M3 m3; printf("%d\n", m3.v);
  Y y(7); printf("%d\n", y.n);
}
