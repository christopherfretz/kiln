// [temp.param]/4 (C++17): a non-type template parameter shall not have
// floating-point type before C++20. Found by the clang negative-test run
// (WP-912).
//@ std: c++17
//@ xfail(kiln): WP-912 follow-up, pre-C++20 non-type template parameter types are not restricted
//@ expect: compile-fail
//@ diag: 8:17 error "a non-type template parameter cannot have type 'float' before C++20"
template <float F> struct S {};
