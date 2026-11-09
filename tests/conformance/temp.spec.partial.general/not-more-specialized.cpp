// [temp.spec.partial.general]/9.2: a partial specialization shall be more
// specialized than the primary template. Found by the clang negative-test
// run (WP-912).
//@ std: c++20
//@ xfail(kiln): WP-912 follow-up, the more-specialized-than-primary check is missing
//@ expect: compile-fail
//@ diag: 9:42 error "class template partial specialization is not more specialized than the primary template"
template <typename T, typename U> struct A {};
template <typename T, typename U> struct A<U, T> {};
