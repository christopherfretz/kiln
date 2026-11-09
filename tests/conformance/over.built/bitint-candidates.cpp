// [over.built]: the built-in operator candidates include _BitInt types, so
// S's conversion to _BitInt(12) picks the built-in operator (clang GH82998).
// From clang's SemaCXX/overload-bitint.cpp. Found by the clang negative-test
// run (WP-912).
//@ std: c++20
//@ xfail(kiln): WP-912 follow-up, _BitInt types are missing from the built-in operator candidates
//@ expect: compile-ok
struct S {
  _BitInt(12) a;
  constexpr operator _BitInt(12)() const { return a; }
};
static_assert(S{10} < 11, "");
static_assert(S{10} + 1 == 11, "");
