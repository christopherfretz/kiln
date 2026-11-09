// [temp.deduct.type]/5.3, CWG526: a non-type template argument that is not
// just the parameter name, such as '(N)', is a non-deduced context. Found by
// the clang negative-test run (WP-912).
//@ std: c++20
//@ xfail(kiln): WP-912 follow-up, parenthesized non-type arguments are deduced
//@ expect: compile-fail
//@ diag: 10:18 error "no matching function for call to 'f'"
template <int> struct S {};
template <int N> void f(S<(N)>);
void g(S<3> s) { f(s); }
