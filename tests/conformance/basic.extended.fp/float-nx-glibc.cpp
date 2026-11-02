// [basic.extended.fp], [dcl.typedef]: glibc declares `typedef double
// _Float32x;` and `typedef long double _Float64x;` for C++ compilers that do
// not report GCC >= 13. kiln reports __GNUC__ 4 (clang's value) and has the
// GCC keywords, so in a system header such a typedef declares nothing and
// the names keep naming GCC's distinct types (spec/pp.md "Predefined
// macros", WP-380). Outside a system header it is an error, as in GCC
// (kiln words it as for _Float32: a type-specifier conflict).
// Reference: GCC 14.
//@ expect: compile-fail
//@ std: c++23
//@ strict-diags
//@ args: -isystem %{test_dir}/Inputs
//@ diag: 23:16 error "cannot combine with previous 'double' declaration specifier"
//@ skip(clang): clang 19 and 22 have no _Float32x or _Float64x (GCC is the reference)
#include <floatnx_typedefs.h>
template <class T, class U> constexpr bool same = false;
template <class T> constexpr bool same<T, T> = true;
static_assert(!same<_Float32x, double> && !same<_Float64x, long double>);
static_assert(same<decltype(1.0f32x), _Float32x>);
#if __GNUC__ != 4
#error "kiln reports clang's __GNUC__"
#endif
typedef double _Float32x;
