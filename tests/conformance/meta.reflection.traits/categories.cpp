// [meta.reflection.traits]/1-3 P2996R13, [meta.unary.cat], [meta.unary.comp]:
// the primary and composite type category traits answer as the builtin
// trait engine's __is_X for the type a reflection represents, a type alias
// included (p4), and std::meta::info is a fundamental scalar type
// ([basic.fundamental]/17).
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
//@ xfail(gcc): compares with clang's __is_* trait builtins, which GCC 16.1 lacks
#include "kiln_meta_traits.h"
using namespace std::meta;

struct C { int m; int f(); };
union U { int a; };
enum E { e0 };
enum class SE { a };
using Alias = const int*;

static_assert(is_void_type(^^void) && is_void_type(^^const void) && !is_void_type(^^int));
static_assert(is_null_pointer_type(^^decltype(nullptr)) && !is_null_pointer_type(^^void*));
static_assert(is_integral_type(^^bool) && is_integral_type(^^char8_t) && !is_integral_type(^^E));
static_assert(is_floating_point_type(^^double) && !is_floating_point_type(^^int));
static_assert(is_array_type(^^int[3]) && is_array_type(^^int[]) && !is_array_type(^^int*));
static_assert(is_pointer_type(^^Alias) && !is_pointer_type(^^int C::*));
static_assert(is_lvalue_reference_type(^^int&) && !is_lvalue_reference_type(^^int&&));
static_assert(is_rvalue_reference_type(^^int&&) && !is_rvalue_reference_type(^^int&));
static_assert(is_member_object_pointer_type(^^int C::*));
static_assert(!is_member_object_pointer_type(^^int (C::*)()));
static_assert(is_member_function_pointer_type(^^int (C::*)()));
static_assert(is_enum_type(^^E) && is_enum_type(^^SE) && !is_enum_type(^^int));
static_assert(is_union_type(^^U) && !is_union_type(^^C));
static_assert(is_class_type(^^C) && !is_class_type(^^U));
static_assert(is_function_type(^^int(int)) && !is_function_type(^^int (*)(int)));
static_assert(is_reflection_type(^^info) && is_reflection_type(^^const info));
static_assert(!is_reflection_type(^^int));

static_assert(is_reference_type(^^int&) && is_reference_type(^^int&&) && !is_reference_type(^^int));
static_assert(is_arithmetic_type(^^float) && is_arithmetic_type(^^char) && !is_arithmetic_type(^^E));
static_assert(is_fundamental_type(^^void) && is_fundamental_type(^^decltype(nullptr)));
static_assert(is_fundamental_type(^^info) && !is_fundamental_type(^^int*));
static_assert(is_object_type(^^int) && is_object_type(^^C) && !is_object_type(^^int&));
static_assert(!is_object_type(^^void) && !is_object_type(^^int()));
static_assert(is_scalar_type(^^E) && is_scalar_type(^^int C::*) && is_scalar_type(^^info));
static_assert(!is_scalar_type(^^C));
static_assert(is_compound_type(^^int*) && is_compound_type(^^C) && !is_compound_type(^^int));
static_assert(is_member_pointer_type(^^int C::*) && !is_member_pointer_type(^^int*));

// The engine is the one consumer: the reflection and the builtin agree.
static_assert(is_scalar_type(^^info) == __is_scalar(info));
static_assert(is_fundamental_type(^^info) == __is_fundamental(info));
