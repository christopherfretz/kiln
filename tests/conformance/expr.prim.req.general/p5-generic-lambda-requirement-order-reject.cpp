// [expr.prim.req.general]/5: the rejected side of
// p5-generic-lambda-requirement-order.cpp. When the lambda's requirement
// holds (`U = HasType`) the next one is substituted, and forming
// `S<int>::type` instantiates `S<int>`, whose error is outside the
// requirement's immediate context: a hard error, not `false`. Found
// running GCC's C++ testsuite.
//@ std: c++20
//@ expect: compile-fail
//@ diag: 10:47 error "type 'int' cannot be used prior to '::' because it has no members"
template <typename T> struct S { using type = T::type; };
struct HasType { using type = int; };
template <typename T> constexpr auto f() {
  return []<typename U>(U) { return requires { typename U::type; typename S<T>::type; }; };
}
bool b = f<int>()(HasType{});
