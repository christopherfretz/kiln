// [temp.arg.nontype]/1: a non-type template argument of class type with
// no implicit conversion to the parameter's type: clang's converted-constant
// wording "value of type ... is not implicitly convertible to ..." at the
// argument (libc++ tuple_size of an incomplete type).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 13:3 error "value of type 'S' is not implicitly convertible to 'int'"
//@ diag: 18:3 error "value of type 'const S' is not implicitly convertible to 'unsigned long'"
//@ requires: clang<22
// clang 19 spelling; class-arg-no-conversion-clang22.cpp has clang 22's (WP-377).
template <int N> struct A {};
struct S {};
A<S{}> a;
template <unsigned long N> struct B {};
struct Inc;
B<sizeof(int *)> ok;
template <class T> struct Size { static constexpr T value{}; };
B<Size<S>::value> b;
