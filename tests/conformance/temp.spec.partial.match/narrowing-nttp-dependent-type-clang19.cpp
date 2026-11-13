// [temp.spec.partial.match]/2, [temp.deduct.type]/20: matching `V<int, 1>`
// against `template <bool B> V<int, B>` fails, since the type of `B`
// (`bool`) differs from the type `T = int` of the corresponding primary
// parameter; `V<int, 2>` fails the same way. Clang 22 rejects the partial
// specialization itself as not more specialized than the primary
// (temp.spec.partial.general/param-typed-primary-converted-arg-not-more-specialized.cpp);
// clang 19, GCC 16.1 and kiln at -fclang-compat=19 accept it. Clang 19
// matched `V<int, 1>` (it does not check the type rule for class templates);
// GCC 16.1 and kiln do not. Split from narrowing-nttp-deduced-types.cpp.
// WP-958; strict matching WP-1405.
//@ std: c++20
//@ requires: clang<22
//@ oracle: gcc
//@ skip(clang): clang 19 matches a partial specialization whose non-type parameter type differs from the primary's
//@ expect: compile-ok
template <class T, T> struct V { static constexpr int v = 0; };
template <bool B> struct V<int, B> { static constexpr int v = 1; };
static_assert(V<int, 1>::v == 0);
static_assert(V<int, 2>::v == 0);
