// [class.inhctor.init]/1: an inherited constructor initializes the base it
// names with the arguments and the rest of the object as a defaulted
// default constructor would (here a default member initializer); /2: a
// virtual base is constructed by the most derived class with the
// inherited constructor's arguments; [namespace.udecl]/3 a dependent base
// named by a template-id (Itanium CI1/CI2 variants).
//@ expect: run
//@ stdout: A 8
//@ stdout: 8 3
//@ stdout: V 5
//@ stdout: 5
//@ stdout: V 6
//@ stdout: 6 4
//@ stdout: 9
extern "C" int printf(const char*, ...);
struct A { A(int x, int y = 7) : s(x + y) { printf("A %d\n", s); } int s; };
struct B : A { using A::A; int z = 3; };
struct V { V(int v) : v(v) { printf("V %d\n", v); } V() : v(-1) {} int v; };
struct M1 : virtual V { using V::V; };
struct M2 : M1 { using M1::M1; int k = 4; };
template <class T> struct Base { Base(T v) : v(v) {} T v; };
template <class T> struct D : Base<T> { using Base<T>::Base; };
int main() {
  B b(1); printf("%d %d\n", b.s, b.z);
  M1 m1(5); printf("%d\n", m1.v);
  M2 m2(6); printf("%d %d\n", m2.v, m2.k);
  D<long> d(9); printf("%ld\n", d.v);
}
