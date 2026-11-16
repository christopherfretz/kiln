// [expr.reflect]/5: a qualified reflection-name is looked up as a name
// first; only a class or enumeration found makes it a type (5.7). A data
// member or static member hiding a base's member type of the same name,
// a namespace-qualified function, class template and function template,
// and a dependent `^^X::x` that names a variable at instantiation.
// Found running GCC's C++ testsuite.
//@ expect: compile-ok
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ oracle: gcc
//@ strict-diags
#include <meta>
struct B { struct S {}; using T = int; };
struct D : B { int S; static int T; };
static_assert(std::meta::is_nonstatic_data_member(^^D::S));
static_assert(std::meta::is_variable(^^D::T));
static_assert(std::meta::is_type(^^B::S));
namespace N { void f(); template <class X> struct TT {}; template <class X> void g(X); }
static_assert(std::meta::is_function(^^N::f));
static_assert(std::meta::is_class_template(^^N::TT));
static_assert(std::meta::is_function_template(^^N::g));
struct V { static constexpr int x = 3; };
template <class X> constexpr auto rx = ^^X::x;
static_assert(std::meta::is_variable(rx<V>));
static_assert(std::meta::is_type(^^typename B::S));
