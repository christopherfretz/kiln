// [namespace.udecl]/19: a using-declarator with a dependent qualifier is
// access-checked when the class is instantiated, at the declarator's name;
// inheriting constructors and protected members named via the class are not errors.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 25:43 error "'f' is a private member of 'A'"
//@ diag: 29:6 note "in instantiation of template class 'V<A>' requested here"
//@ diag: 20:34 note "implicitly declared private here"
//@ diag: 25:55 error "'v' is a private member of 'A'"
//@ diag: 20:43 note "implicitly declared private here"
//@ diag: 25:88 error "'type' is a private member of 'A'"
//@ diag: 20:23 note "implicitly declared private here"
//@ diag: 24:43 error "'f' is a private member of 'B'"
//@ diag: 30:6 note "in instantiation of template class 'D<B>' requested here"
//@ diag: 21:16 note "implicitly declared private here"
//@ diag: 26:52 error "'operator()' is a private member of 'C'"
//@ diag: 32:6 note "in instantiation of template class 'P<C>' requested here"
//@ diag: 23:26 note "declared private here"
template<class T> struct D;
class A { typedef int type; void f(); int v; protected: int p; };
class B { void f(); public: void f(int); };
class E { friend struct D<E>; void f(); };
struct C { private: void operator()(); };
template<class T> struct D : T { using T::f; };
template<class T> struct V : T { using T::f; using T::v; using T::p; using typename T::type; };
template<class... Ts> struct P : Ts... { using Ts::operator()...; };
template<class T> struct I : T { I(); using T::T; };
class F { F(int); public: F(); };
V<A> va;
D<B> db;
D<E> de;
P<C> pc;
I<F> i;
