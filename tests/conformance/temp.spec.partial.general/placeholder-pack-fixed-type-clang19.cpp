// [temp.spec.partial.general]/9.2: clang 19 accepted a partial
// specialization giving an `auto...` primary pack a fixed type
// (`int...`), treating it as more specialized; -fclang-compat=19 keeps
// that, and the partial is chosen for `int` arguments. Clang 22 rejects it
// (placeholder-pack-fixed-type-not-more-specialized.cpp, WP-1105).
//@ std: c++20
//@ requires: clang<22
//@ expect: compile-ok
//@ strict-diags
template <auto... Vs> struct S { static constexpr int v = 0; };
template <int... Is> struct S<Is...> { static constexpr int v = 1; };
static_assert(S<1, 2>::v == 1);
static_assert(S<1, nullptr>::v == 0);
