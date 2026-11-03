// [dcl.spec.auto.general]/11: a variable declared with a deduced type
// cannot be used in its own initializer. Clang 19 calls a variable
// template explicit specialization a `variable` and does not diagnose a
// partial specialization's use of itself (own-init-var-template.cpp is
// the clang 22 wording).
//@ std: c++26
//@ requires: clang<22
//@ expect: compile-fail
//@ strict-diags
//@ diag: 14:10 error "variable 'x' declared with deduced type 'auto' cannot appear in its own initializer"
//@ diag: 17:27 error "variable 'ap' declared with deduced type 'auto' cannot appear in its own initializer"
//@ diag: 19:42 error "variable 'm' declared with deduced type 'const auto' cannot appear in its own initializer"
//@ diag: 20:21 error "variable 'y' declared with deduced type 'auto' cannot appear in its own initializer"
auto x = x;
template<class T> auto ap = 0;
template<class T> auto ap<T*> = ap<T*>;
template<> auto ap<int> = ap<int>;
struct S { template<class T> static constexpr auto m = 0; };
template<> constexpr auto S::m<int> = S::m<int>;
void f() { auto y = y; }
