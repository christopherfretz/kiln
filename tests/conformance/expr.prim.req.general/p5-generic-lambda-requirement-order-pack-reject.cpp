// [expr.prim.req.general]/5: the rejected side of
// p5-generic-lambda-requirement-order-pack.cpp. When the lambda's
// requirement holds (`U = HasType`) the next one is substituted for the
// element's `Ts`, and forming `S<int>::type` instantiates `S<int>`, whose
// error is outside the requirement's immediate context: a hard error, not
// `false`. Found running GCC's C++ testsuite (WP-1599).
//@ std: c++26
//@ expect: compile-fail
//@ diag: 10:47 error "type 'int' cannot be used prior to '::' because it has no members"
template <typename T> struct S { using type = T::type; };
struct HasType { using type = int; };
template <typename... Ts> constexpr int fold(auto u) {
  return (0 + ... + ([]<typename U>(U) { return requires { typename U::type; typename S<Ts>::type; }; }(u) ? 1 : 0));
}
int x = fold<HasType, int>(HasType{});
