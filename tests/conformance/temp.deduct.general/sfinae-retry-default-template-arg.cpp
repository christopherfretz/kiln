// [temp.deduct.general]/8 (WP-3697): a function template specialization
// whose signature failed for too few template arguments is re-deduced
// once a later redeclaration adds a default template argument.
//@ expect: compile-ok
template <class T, class U> struct Box {};
template <class... Ts> auto g(Ts...) -> decltype(Box<Ts...>{}, 0);
long g(...);

long p = sizeof(g(0));
static_assert(sizeof(g(0)) == sizeof(long));

template <template <class...> class TT, class T> auto k(T) -> decltype(TT<T>{}, 0);
template <template <class...> class TT> long k(...);
static_assert(sizeof(k<Box>(0)) == sizeof(long));

template <class T, class U = int> struct Box;

static_assert(sizeof(g(0)) == sizeof(int));
static_assert(sizeof(k<Box>(0)) == sizeof(int));
