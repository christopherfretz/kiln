// [clang ext] [temp.deduct.general]/8 A type written in an address space
// over a template parameter whose argument is already in a different
// space is a substitution failure in the immediate context ("conflicting
// address space qualifiers", clang `TreeTransform::RebuildQualifiedType`):
// a partial specialization does not match, a function template is not
// viable, a requirement is not satisfied; the same space or none is
// valid. spec/sema.md "Address spaces", WP-1914.
//@ expect: compile-ok
//@ strict-diags
template <class T, class = void> struct D {
  static const bool v = false;
};
template <class T>
struct D<T, decltype((void)(T __attribute__((address_space(1))) *)nullptr)> {
  static const bool v = true;
};
static_assert(!D<__attribute__((address_space(2))) int>::v);
static_assert(D<__attribute__((address_space(1))) int>::v);
static_assert(D<int>::v);
template <class T> auto h(T *) -> T __attribute__((address_space(1))) *;
template <class T> int h(...);
static_assert(__is_same(decltype(h<__attribute__((address_space(2))) int>(0)), int));
static_assert(__is_same(decltype(h<int>(0)), int __attribute__((address_space(1))) *));
template <class T>
concept C = requires { (T __attribute__((address_space(1))) *)nullptr; };
static_assert(!C<__attribute__((address_space(2))) int>);
static_assert(C<__attribute__((address_space(1))) int>);
static_assert(C<int>);
template <int N, class T>
concept CN = requires { (T __attribute__((address_space(N))) *)nullptr; };
static_assert(!CN<1, __attribute__((address_space(2))) int>);
static_assert(CN<2, int>);
