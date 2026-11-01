// Mangling differential: inheriting constructors are mangled as
// CI1/CI2 followed by the inherited-from base's type ([class.inhctor.init];
// Itanium ABI <ctor-dtor-name> ::= CI1 <type> | CI2 <type>).
struct A { A(int, int = 7); A(double); int s; };
struct B : A { using A::A; int z = 3; };
struct V { V(int); V(); };
struct M1 : virtual V { using V::V; };
struct M2 : M1 { using M1::M1; };
namespace ns { template <class T> struct Base { Base(T, T*) {} }; }
template <class T> struct D : ns::Base<T> { using ns::Base<T>::Base; };
template <class T> struct W : T { using T::T; };
struct Q { Q(int, Q*, const Q&); };
struct R : Q { using Q::Q; };
namespace ns { struct P { P(P*); }; struct S : P { using P::P; }; }
// A constructor template specialization: CI1/CI2 <base> <template-args>
// with the pattern's parameter types (WP-146); its base variant is C2.
struct VB { template <class... T> VB(T...); };
struct VD : VB { using VB::VB; };
struct VE : VB { VE() : VB(1, 2.0) {} };
template <class T> struct TB { template <class U> TB(U, T*); };
struct TD : TB<int> { using TB<int>::TB; };
void use() {
  R r(1, nullptr, r); ns::S s(nullptr);
  B b(1); B b2(2.0);
  M1 m1(5); M2 m2(6);
  long l = 0; D<long> d(9, &l);
  W<A> w(3); W<W<A>> ww(4.0);
  VD vd(1, 'c'); VE ve; TD td(2L, nullptr);
}
