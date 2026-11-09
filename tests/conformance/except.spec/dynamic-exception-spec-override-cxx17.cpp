// C++17 [except.spec]/8 (C++14 [except.spec]/5): an overrider's dynamic
// list, kept after `-Wdynamic-exception-spec`, must allow no more than the
// base's, type by type; a dependent one is checked at the definition and
// again in each specialization that fails.  WP-1137.
//@ std: c++17
//@ expect: compile-fail
//@ args: -ferror-limit=0
//@ strict-diags
//@ diag: 34:29 error "ISO C++17 does not allow dynamic exception specifications"
//@ diag: 34:29 note "use 'noexcept(false)' instead"
//@ diag: 35:43 error "ISO C++17 does not allow dynamic exception specifications"
//@ diag: 35:43 note "use 'noexcept(false)' instead"
//@ diag: 35:39 error "exception specification of overriding function is more lax than base version"
//@ diag: 34:25 note "overridden virtual function is here"
//@ diag: 35:39 error "exception specification of overriding function is more lax than base version"
//@ diag: 36:9 note "in instantiation of template class 'D<char>' requested here"
//@ diag: 34:25 note "overridden virtual function is here"
//@ diag: 39:41 error "exception specification of overriding function is more lax than base version"
//@ diag: 42:7 note "in instantiation of template class 'D2<F>' requested here"
//@ diag: 38:26 note "overridden virtual function is here"
//@ diag: 44:45 error "ISO C++17 does not allow dynamic exception specifications"
//@ diag: 44:45 note "use 'noexcept(false)' instead"
//@ diag: 44:41 error "exception specification of overriding function is more lax than base version"
//@ diag: 38:26 note "overridden virtual function is here"
//@ diag: 44:41 error "exception specification of overriding function is more lax than base version"
//@ diag: 45:9 note "in instantiation of template class 'D3<int>' requested here"
//@ diag: 38:26 note "overridden virtual function is here"
//@ diag: 46:26 error "ISO C++17 does not allow dynamic exception specifications"
//@ diag: 46:26 note "use 'noexcept(false)' instead"
//@ diag: 46:22 error "exception specification of overriding function is more lax than base version"
//@ diag: 34:25 note "overridden virtual function is here"
//@ diag: 47:26 error "ISO C++17 does not allow dynamic exception specifications"
//@ diag: 47:26 note "use 'noexcept(false)' instead"
struct B { virtual void k() throw(int); };
template<class T> struct D : B { void k() throw(T); };
D<char> e;
D<int> e2;
struct B2 { virtual void k() noexcept; };
template<class T> struct D2 : B2 { void k() noexcept(T::v); };
struct F { static const bool v = false; };
struct Tr { static const bool v = true; };
D2<F> f;
D2<Tr> f2;
template<class T> struct D3 : B2 { void k() throw(T); };
D3<int> g3;
struct D4 : B { void k() throw(char); };
struct D5 : B { void k() throw(int); };
