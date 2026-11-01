// [lex.icon]/3 (C++23, P0330R8): the `z`/`Z` suffix gives the signed
// integer type corresponding to `std::size_t`, and `uz` gives `size_t`.
// WP-185 feature probe.
//@ expect: compile-ok
//@ std: c++23
//@ strict-diags
template <class T, class U> constexpr bool same = false;
template <class T> constexpr bool same<T, T> = true;
using size_t = decltype(sizeof 0);
static_assert(same<decltype(0uz), size_t>);
static_assert(same<decltype(42UZ), size_t>);
static_assert(same<decltype(7zu), size_t>);
static_assert(same<decltype(1z), __make_signed(size_t)>);
static_assert(1z + 2Z == 3);
