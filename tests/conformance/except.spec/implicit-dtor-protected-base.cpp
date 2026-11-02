// [except.spec]/8, [meta.unary.prop] is_nothrow_destructible: an implicit
// (or unwritten-specification) destructor is potentially-throwing only if
// a subobject's destructor is, whatever that destructor's access. A
// protected base destructor (libc++'s `__list_imp` under `std::list`)
// leaves the derived class nothrow-destructible (WP-309).
//@ expect: compile-ok
//@ strict-diags
struct M { ~M() {} };
struct Throws { ~Throws() noexcept(false); };
template <class T> struct B { M m; protected: ~B(); };
template <class T> B<T>::~B() {}
template <class T> struct D : private B<T> {};
struct B2 { protected: ~B2(); };
struct D2 : private B2 {};
struct D3 : public B2 { M arr[2]; };
struct B4 { protected: ~B4() noexcept(false); };
struct D4 : B4 {};
struct D5 { Throws t; };
struct P { private: ~P(); };
struct D6 : P {}; // implicitly deleted destructor ([class.dtor]/7)

static_assert(__is_nothrow_destructible(D<int>));
static_assert(__is_nothrow_destructible(D2));
static_assert(__is_nothrow_destructible(D3));
static_assert(!__is_nothrow_destructible(B2)); // not accessible itself
static_assert(!__is_nothrow_destructible(D4));
static_assert(__is_destructible(D4));
static_assert(!__is_nothrow_destructible(D5));
static_assert(!__is_destructible(D6) && !__is_nothrow_destructible(D6));
