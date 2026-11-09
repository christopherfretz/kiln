// Mangling differential: the qualifier of a member access in a dependent
// signature ([temp.over.link]/5, [basic.lookup.qual.general]/2;
// spec/abi.md "Mangling", WP-1029). Behind an explicit object of
// dependent type clang keeps the qualifier as spelled, every level by
// name: `t.T::f()` is `sr1TE1f` (no substitution for `T`),
// `t.A::Self::f()` with typedef `A` is `sr1A4SelfE`, and spellings that
// name the same class are not equivalent. Elsewhere a typedef qualifier
// mangles its own name, a namespace alias its alias name, a substituted
// template parameter is an unresolved type (`srS0_1m`) and a class
// template specialization keeps its arguments (`sr1BIiEE1f`).
namespace qm {
struct P { int m; int f(); using Self = P; };
namespace N { struct Q { int f(); }; }
struct R : N::Q, P {};
template <class T> auto q1(T t) -> decltype(t.T::f());
template <class T> auto q2(T t) -> decltype(t.P::f());
template <class T> auto q3(T t) -> decltype(t.N::Q::f());
template <class T> auto q4(T t) -> decltype(t.T::Self::f());
template <class T> auto q5(T t) -> decltype(t.T::m);
template <class T> auto q6(T *t) -> decltype(t->T::m);
template <class T> auto q7(T t) -> decltype(t.T::f() + t.T::m);
template <class T, class U> auto q8(T t, U) -> decltype(t.U::m);
void use() { P p; R r; q1(p); q2(p); q3(r); q4(p); q5(p); q6(&p); q7(p); q8(r, p); }
} // namespace qm
namespace qm2 {
struct P { int m; int f(); using Self = P; };
using A = P;
template <class> struct B { static int f(); int m; };
struct BB : B<int>, P {};
template <class T> struct C : T {
  template <class U> auto h(U u) -> decltype(this->T::m + u);
  template <class U> auto k(U u) -> decltype(T::m + u);
};
P gp;
template <class T> auto q1(T t) -> decltype(t.A::f());
template <class T> auto q2(BB t, T u = 0) -> decltype(t.B<T>::f() + u);
template <class T> auto q3(T t) -> decltype(gp.T::m);
template <class T> auto q4(T t) -> decltype(t.T::Self::Self::f());
template <class T> auto q5(T t) -> decltype(t.::qm2::P::f());
void use() { P p; BB bb; C<P> c; c.h(1); c.k(1); q1(p); q2<int>(bb, 0); q3(p); q4(p); q5(p); }
} // namespace qm2
namespace qm3 {
template <class> struct B { static int f(); int m; };
struct BB : B<int> {};
template <class T> auto q2(T t) -> decltype(t.template B<int>::f());
template <class T> auto q6(T t) -> decltype(t.B<int>::f());
void use() { BB bb; q2(bb); q6(bb); }
} // namespace qm3
namespace qm5 {
struct P { int m; int f(); using Self = P; };
template <class> struct B { static int f(); int m; };
using BI = B<int>;
struct BB : B<int>, P {};
template <class T> struct C : T {
  using Base = T;
  template <class U> auto h(U u) -> decltype(this->T::m + u);
  template <class U> auto k(U u) -> decltype(T::m + u);
  template <class U> auto j(U u) -> decltype(this->Base::m + u);
  template <class U> auto i(U u) -> decltype(T::Self::m + u);
};
template <class T> auto q6(T t) -> decltype(t.B<int>::f());
template <class T> auto q7(T t) -> decltype(t.BI::f());
void use() { P p; BB bb; C<P> c; c.h(1); c.k(1); c.j(1); c.i(1); q6(bb); q7(bb); }
} // namespace qm5
namespace qm7 {
struct P { int m; int f(); using Self = P; template <class> using Al = P; };
using A = P;
template <class T> struct C : T {
  template <class U> auto h(U u) -> decltype(this->P::Self::m + u);
};
template <class T> auto q1(T t) -> decltype(t.P::Self::f());
template <class T> auto q2(T t) -> decltype(t.A::Self::f());
template <class T> auto q3(T *t) -> decltype(t->A::m);
void use() { P p; C<P> c; c.h(1); q1(p); q2(p); q3(&p); }
} // namespace qm7
namespace qm8 {
struct P { int m; int f(); using Self = P; template <class U> static int g(U); };
using A = P;
template <class T> struct C : T {
  using Base = T;
  template <class U> auto k(U u) -> decltype(Base::m + u);
  template <class U> auto k2(U u) -> decltype(this->Base::m + u);
  template <class U> auto k3(U u) -> decltype(this->A::m + u);
  template <class U> auto k4(U u) -> decltype(this->P::m + u);
};
template <class U> auto r1(U u) -> decltype(P::Self::template g<U>(u));
template <class U> auto r2(U u) -> decltype(A::template g<U>(u));
template <class U> auto r3(U u) -> decltype(P::g(u));
void use() { P p; C<P> c; c.k(1); c.k2(1); c.k3(1); c.k4(1); r1(1); r2(1); r3(1); }
} // namespace qm8
namespace qm9 {
struct P { int m; int f(); using Self = P; using Other = P; struct N { static int g(); int f(); }; };
using A = P;
namespace ns { using P = ::qm9::P; }
template <class T> auto q1(T t) -> decltype(t.A::Self::f());
template <class T> auto q2(T t) -> decltype(t.A::Other::f());
template <class T> auto q3(T t) -> decltype(t.P::Self::f());
template <class T> auto q4(T t) -> decltype(t.ns::P::f());
template <class T> auto q5(T t) -> decltype(t.ns::P::Self::f());
template <class T> auto q6(T t) -> decltype(t.P::N::f());
template <class T> auto q7(T t) -> decltype(t.A::f());
template <class T> auto q8(T t) -> decltype(t.P::f());
template <class T> auto q9(T t) -> decltype(t.A::m);
void use() { P p; q1(p); q2(p); q3(p); q4(p); q5(p); q7(p); q8(p); q9(p); }
} // namespace qm9
namespace qm11 {
struct P { int m; int f(); using Self = P; };
namespace ns { using P = ::qm11::P; }
namespace ns2 { using P = ::qm11::P; }
namespace na = ns;
template <class T> auto q1(T t) -> decltype(t.ns::P::f());
template <class T> auto q2(T t) -> decltype(t.ns2::P::f());
template <class T> auto q3(T t) -> decltype(t.na::P::f());
template <class T> auto q4(T t) -> decltype(t.ns::P::Self::f());
template <class T> auto q5(T t) -> decltype(t.ns2::P::Self::f());
void use() { P p; q1(p); q2(p); q3(p); q4(p); q5(p); }
} // namespace qm11
namespace qm12 {
namespace ns { struct P { template <class U> static int g(U); }; template <class U> int h(U); }
namespace na = ns;
template <class U> auto r1(U u) -> decltype(na::P::g(u));
template <class U> auto r2(U u) -> decltype(na::h(u));
template <class U> auto r3(U u) -> decltype(ns::h(u) + na::P::g(u));
void use() { r1(1); r2(1); r3(1); }
} // namespace qm12
