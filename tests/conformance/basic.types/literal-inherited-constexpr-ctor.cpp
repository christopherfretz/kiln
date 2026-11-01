// [basic.types.general]/10.5.2: a class with a constexpr constructor or
// constructor template inherited from a base (`using B::B;`) that is not a
// copy or move constructor is a literal type (libc++ variant's `__dtor`,
// whose constructors are all inherited), and so can be a constexpr
// variable's type. Inheriting only non-constexpr constructors does not help,
// even when the base's copy constructor is constexpr.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 28:13 error "constexpr variable cannot have non-literal type 'const E'"
//@ diag: 24:8 note "'E' is not literal because it is not an aggregate and has no constexpr constructors other than copy or move constructors"
struct B { int i; constexpr B(int x) : i(x) {} };
struct D : B { using B::B; D(const D&) = default; };
static_assert(__is_literal_type(D));
constexpr D d(3);
static_assert(d.i == 3);
template <class T> struct BT { T i; template <class U> constexpr BT(U x) : i(x) {} };
template <class T> struct DT : BT<T> { using BT<T>::BT; DT(const DT&) = default; };
static_assert(__is_literal_type(DT<int>));
constexpr DT<int> dt('a');
static_assert(dt.i == 'a');
struct C { int i; constexpr C(const C& c) : i(c.i) {} C(int x) : i(x) {} };
struct EC : C { using C::C; EC(const EC&) = default; };
static_assert(!__is_literal_type(EC));
struct E : C { E(int x) : C(x) {} using C::C; };
static_assert(!__is_literal_type(E));
struct F : B { using B::B; F(const F&) = default; };
static_assert(__is_literal_type(F));
constexpr E e(1);
