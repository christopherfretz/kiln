// [temp.spec.partial.match]/2, [temp.deduct.type]/17: a value deduced for a
// non-type parameter whose type is a preceding type parameter is converted
// to that type, so `V<int, 2>` does not match `V<int, B>` with `bool B`.
// Clang 22 rejects the partial specialization itself as not more
// specialized than the primary (-Winvalid-partial-specialization); clang 19
// and kiln accept it. Split from narrowing-nttp-deduced-types.cpp. WP-958.
//@ std: c++20
//@ requires: clang<22
//@ expect: compile-ok
template <class T, T> struct V { static constexpr int v = 0; };
template <bool B> struct V<int, B> { static constexpr int v = 1; };
static_assert(V<int, 1>::v == 1);
static_assert(V<int, 2>::v == 0);
