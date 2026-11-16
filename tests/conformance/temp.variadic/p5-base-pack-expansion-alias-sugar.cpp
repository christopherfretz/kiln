// [temp.variadic]/5, [temp.alias]/2: in `struct env : as_base<Es, 1>...`
// the pattern names the pack `Es` only through an alias template that
// ignores its parameter, so the base-specifier is still a pack expansion,
// expanded at instantiation into one base per element. A pack of lambdas
// in the bases gives distinct closure types, one per element.
// Found running GCC's C++ testsuite. WP-1509.
//@ expect: compile-ok
struct tt {};
template <class C, int N> using as_base = tt;
template <class... Es>
struct env : as_base<Es, 1>... {};
env<int> e1;
static_assert(__is_base_of(tt, env<int>));

template <class Slot, auto Tag = [] {}> constexpr int counter = 1;
template <class... Es>
struct env2 : as_base<Es, counter<int>>... {};
env2<int> e2;
static_assert(__is_base_of(tt, env2<int>));

template <class... Es>
struct lam : decltype([](Es) {})... {};
static_assert(__is_empty(lam<int, int, char*>));
