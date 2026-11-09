// [support.srclocation.cons]: a __builtin_LINE() default argument takes the
// line of the call, which for a member call is the member name's line (clang
// 22, GH119129). From clang's SemaCXX/source_location.cpp. Found by the
// clang negative-test run (WP-912).
//@ std: c++20
//@ requires: clang>=22
//@ expect: compile-ok
struct X {
  constexpr int foo(int l = __builtin_LINE()) { return l; }
};
static_assert(X{}.foo() == __LINE__);
static_assert(X{}.
              foo() == __LINE__);
