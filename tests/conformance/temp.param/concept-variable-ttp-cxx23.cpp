// [temp.param]/1 (P2841R7) is new in C++26: in C++23 mode a
// concept-tt-parameter or variable-tt-parameter is diagnosed as a C++2c
// extension, with clang 22's wording (spec/parse.md "Template template
// parameter kinds"). WP-746.
//@ expect: compile-fail
//@ strict-diags
//@ requires: clang>=22
//@ args: -std=c++23
//@ diag: 15:28 error "concept template parameter is a C++2c extension"
//@ diag: 16:28 error "concept template parameter is a C++2c extension"
//@ diag: 17:28 error "variable template template parameter is a C++2c extension"
//@ diag: 18:28 error "variable template template parameter is a C++2c extension"
template <class T> concept Int = __is_same(T, int);
template <class T> constexpr int sz = sizeof(T);
template <template <class> concept C> struct A {};
template <template <class> concept... Cs> struct P {};
template <template <class> auto V> struct B {};
template <template <class> auto> struct U {};
A<Int> a;
B<sz> b;
