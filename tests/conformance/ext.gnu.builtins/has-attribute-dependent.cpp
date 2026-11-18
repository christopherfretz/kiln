// __builtin_has_attribute with an operand that depends on a template
// parameter is rejected when the template is defined, as GCC 16.1 does
// ("sorry, unimplemented: '__builtin_has_attribute' with dependent argument
// not supported yet"), as is a class member access there, dependent or not
// (GCC's refusal too). Another non-dependent operand is answered there.
// Found running GCC's C++ testsuite. WP-1611.
//@ std: c++20
//@ expect: compile-fail
//@ skip(clang): clang 19/22 have no __builtin_has_attribute
//@ strict-diags
//@ diag: 21:41 error "'__builtin_has_attribute' with a dependent operand or attribute argument is not supported"
//@ diag: 20:38 error "'__builtin_has_attribute' with a dependent operand or attribute argument is not supported"
//@ diag: 17:38 error "'__builtin_has_attribute' with a dependent operand or attribute argument is not supported"
//@ diag: 18:36 error "'__builtin_has_attribute' with a dependent operand or attribute argument is not supported"
struct __attribute__((aligned(16))) A { int a; };

template <class T> bool g() { return __builtin_has_attribute(T, aligned); }
template <int N> bool h() { return __builtin_has_attribute(int, aligned(N)); }
template <class T> bool k() { return __builtin_has_attribute(A, aligned(16)); }
template <int> bool m(A *p) { return __builtin_has_attribute(p->a, aligned); }
template <int> struct C { static_assert(__builtin_has_attribute(((A *)0)->a, aligned)); };
static_assert(__builtin_has_attribute(A, aligned(16)));
bool b = k<int>();
