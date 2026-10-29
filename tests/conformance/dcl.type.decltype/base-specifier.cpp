// [class.derived.general]/1: a class-or-decltype may be a decltype-specifier;
// its type must denote a class ([class.derived.general]/2).
//@ strict-diags
struct A { int a; };
struct B { int b; };
A make_a();
B make_b();
struct D : decltype(make_a()), public decltype(make_b()) {};
static_assert(__is_base_of(A, D));
static_assert(__is_base_of(B, D));
static_assert(sizeof(D) == 2 * sizeof(int));
template <class T> struct H { static T get(); };
template <class T> struct E : decltype(H<T>::get()) {};
static_assert(__is_base_of(A, E<A>));
static_assert(!__is_base_of(A, E<B>));
static_assert(sizeof(E<B>) == sizeof(int));
