// -Wno-static-inline-explicit-instantiation silences the warning for the
// ignored GNU keyword (WP-1495).
//@ expect: compile-ok
//@ std: c++17
//@ args: -Wno-static-inline-explicit-instantiation
//@ strict-diags
template<class T> struct A {};
static template struct A<int>;
inline template struct A<char>;
