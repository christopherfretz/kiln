// [except.spec]/8, /13: the implicit exception specification of an overrider
// (a destructor without a noexcept-specifier, an implicit destructor, a
// defaulted one, or an instantiated dependent noexcept-specifier) is computed
// and must not be laxer than the base's, which is resolved too (WP-171).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 27:16 error "exception specification of overriding function is more lax than base version"
//@ diag: 26:16 note "overridden virtual function is here"
//@ diag: 28:21 error "exception specification of overriding function is more lax than base version"
//@ diag: 24:20 note "overridden virtual function is here"
//@ diag: 29:8 error "exception specification of overriding function is more lax than base version"
//@ diag: 24:20 note "overridden virtual function is here"
//@ diag: 30:27 error "exception specification of overriding function is more lax than base version"
//@ diag: 31:17 note "in instantiation of template class 'X<M>' requested here"
//@ diag: 24:20 note "overridden virtual function is here"
//@ diag: 32:40 error "exception specification of overriding function is more lax than base version"
//@ diag: 33:17 note "in instantiation of template class 'Y<M>' requested here"
//@ diag: 24:20 note "overridden virtual function is here"
//@ diag: 34:40 error "exception specification of overriding function is more lax than base version"
//@ diag: 36:11 note "in instantiation of template class 'V<Throws>' requested here"
//@ diag: 24:39 note "overridden virtual function is here"
//@ diag: 37:23 error "exception specification of overriding function is more lax than base version"
//@ diag: 24:20 note "overridden virtual function is here"
struct B { virtual ~B(); virtual void f() noexcept; };
struct M { ~M() noexcept(false); };
struct F : B { ~F() override; };
struct G : F { ~G() noexcept(false) override; };
struct D : B { M m; ~D() override; };
struct E : B { M m; };
template <class T> struct X : B { T t; };
X<int> xi; X<M> xm;
template <class T> struct Y : B { T t; ~Y() override = default; };
Y<int> yi; Y<M> ym;
template <class T> struct V : B { void f() noexcept(T::v) override; };
struct Throws { static constexpr bool v = false; };
V<Throws> vt;
struct Q : B { struct N : B { M m; }; };
