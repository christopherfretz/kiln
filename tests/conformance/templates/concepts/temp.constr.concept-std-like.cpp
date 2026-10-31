//@ expect: compile-ok
//@ xfail(kiln): `__is_convertible` is not implemented (WP-068a); this passed only through the WP-073 "assumed satisfied" fallback that WP-109 removed
// [concepts.lang]-style concepts over type traits.
template <class T, class U> concept same_as = __is_same(T, U) && __is_same(U, T);
template <class From, class To> concept convertible_to =
    __is_convertible(From, To) && requires { static_cast<To>(*(From *)nullptr); };
template <class D, class B> concept derived_from =
    __is_base_of(B, D) && __is_convertible(const volatile D *, const volatile B *);
template <class T> concept integral = __is_integral(T);
struct Base {};
struct Derived : Base {};
static_assert(same_as<int, int>);
static_assert(!same_as<int, long>);
static_assert(convertible_to<int, long>);
static_assert(!convertible_to<int *, long>);
static_assert(derived_from<Derived, Base>);
static_assert(integral<char> && !integral<float>);
