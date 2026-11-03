// [dcl.spec.auto.general]/11: a variable declared with a deduced type
// cannot be used in its own initializer. Clang 22 names a variable
// template, its partial specialization and its explicit specialization
// in that wording (`variable template partial specialization 'ap<T *>'`);
// the use of a specialization of the primary template is diagnosed once,
// in the pattern.
//@ std: c++26
//@ requires: clang>=22
//@ expect: compile-fail
//@ strict-diags
//@ diag: 17:10 error "variable 'x' declared with deduced type 'auto' cannot appear in its own initializer"
//@ diag: 18:28 error "variable template 't' declared with deduced type 'auto' cannot appear in its own initializer"
//@ diag: 21:33 error "variable template partial specialization 'ap<T *>' declared with deduced type 'auto' cannot appear in its own initializer"
//@ diag: 22:27 error "variable template explicit specialization 'ap<int>' declared with deduced type 'auto' cannot appear in its own initializer"
//@ diag: 24:42 error "variable template explicit specialization 'm<int>' declared with deduced type 'const auto' cannot appear in its own initializer"
//@ diag: 25:21 error "variable 'y' declared with deduced type 'auto' cannot appear in its own initializer"
auto x = x;
template<class T> auto t = t<T>;
int u = t<int>;
template<class T> auto ap = 0;
template<class T> auto ap<T*> = ap<T*>;
template<> auto ap<int> = ap<int>;
struct S { template<class T> static constexpr auto m = 0; };
template<> constexpr auto S::m<int> = S::m<int>;
void f() { auto y = y; }
