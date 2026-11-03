// kiln-internal std::meta [meta.reflection.traits] surface (-freflection;
// spec/reflection-traits.md). Not <meta> (WP-484): the declarations the
// trait intrinsics answer to. Every function with no body is a consteval
// intrinsic the Oven dispatches by qualified name and signature
// (spec/reflection.md §3.1); each is one query of the builtin trait engine
// (the one `__is_X(T)` uses).
#pragma once

#include "kiln_meta.h"

namespace std::meta {
using __kiln_size_t = decltype(sizeof 0);
// [meta.reflection.traits] primary type categories
consteval bool is_void_type(info type);
consteval bool is_null_pointer_type(info type);
consteval bool is_integral_type(info type);
consteval bool is_floating_point_type(info type);
consteval bool is_array_type(info type);
consteval bool is_pointer_type(info type);
consteval bool is_lvalue_reference_type(info type);
consteval bool is_rvalue_reference_type(info type);
consteval bool is_member_object_pointer_type(info type);
consteval bool is_member_function_pointer_type(info type);
consteval bool is_enum_type(info type);
consteval bool is_union_type(info type);
consteval bool is_class_type(info type);
consteval bool is_function_type(info type);
consteval bool is_reflection_type(info type);

// composite type categories
consteval bool is_reference_type(info type);
consteval bool is_arithmetic_type(info type);
consteval bool is_fundamental_type(info type);
consteval bool is_object_type(info type);
consteval bool is_scalar_type(info type);
consteval bool is_compound_type(info type);
consteval bool is_member_pointer_type(info type);

// type properties
consteval bool is_const_type(info type);
consteval bool is_volatile_type(info type);
consteval bool is_trivially_copyable_type(info type);
consteval bool is_standard_layout_type(info type);
consteval bool is_empty_type(info type);
consteval bool is_polymorphic_type(info type);
consteval bool is_abstract_type(info type);
consteval bool is_final_type(info type);
consteval bool is_aggregate_type(info type);
consteval bool is_structural_type(info type);
consteval bool is_signed_type(info type);
consteval bool is_unsigned_type(info type);
consteval bool is_bounded_array_type(info type);
consteval bool is_unbounded_array_type(info type);
consteval bool is_scoped_enum_type(info type);

// supported operations
consteval bool is_default_constructible_type(info type);
consteval bool is_copy_constructible_type(info type);
consteval bool is_move_constructible_type(info type);
consteval bool is_assignable_type(info a, info b);
consteval bool is_copy_assignable_type(info type);
consteval bool is_move_assignable_type(info type);
consteval bool is_swappable_with_type(info a, info b);
consteval bool is_swappable_type(info type);
consteval bool is_destructible_type(info type);
consteval bool is_trivially_default_constructible_type(info type);
consteval bool is_trivially_copy_constructible_type(info type);
consteval bool is_trivially_move_constructible_type(info type);
consteval bool is_trivially_assignable_type(info a, info b);
consteval bool is_trivially_copy_assignable_type(info type);
consteval bool is_trivially_move_assignable_type(info type);
consteval bool is_trivially_destructible_type(info type);
consteval bool is_nothrow_default_constructible_type(info type);
consteval bool is_nothrow_copy_constructible_type(info type);
consteval bool is_nothrow_move_constructible_type(info type);
consteval bool is_nothrow_assignable_type(info a, info b);
consteval bool is_nothrow_copy_assignable_type(info type);
consteval bool is_nothrow_move_assignable_type(info type);
consteval bool is_nothrow_swappable_with_type(info a, info b);
consteval bool is_nothrow_swappable_type(info type);
consteval bool is_nothrow_destructible_type(info type);
consteval bool is_implicit_lifetime_type(info type);
consteval bool has_virtual_destructor(info type);
consteval bool has_unique_object_representations(info type);
consteval bool reference_constructs_from_temporary(info a, info b);
consteval bool reference_converts_from_temporary(info a, info b);

// type property queries
consteval __kiln_size_t rank(info type);
consteval __kiln_size_t extent(info type, unsigned i = 0);

// type relations
consteval bool is_same_type(info a, info b);
consteval bool is_base_of_type(info a, info b);
consteval bool is_virtual_base_of_type(info a, info b);
consteval bool is_convertible_type(info a, info b);
consteval bool is_nothrow_convertible_type(info a, info b);
consteval bool is_layout_compatible_type(info a, info b);
consteval bool is_pointer_interconvertible_base_of_type(info a, info b);

// const-volatile, reference, sign, array and pointer modifications;
// other transformations
consteval info remove_const(info type);
consteval info remove_volatile(info type);
consteval info remove_cv(info type);
consteval info add_const(info type);
consteval info add_volatile(info type);
consteval info add_cv(info type);
consteval info remove_reference(info type);
consteval info add_lvalue_reference(info type);
consteval info add_rvalue_reference(info type);
consteval info make_signed(info type);
consteval info make_unsigned(info type);
consteval info remove_extent(info type);
consteval info remove_all_extents(info type);
consteval info remove_pointer(info type);
consteval info add_pointer(info type);
consteval info remove_cvref(info type);
consteval info decay(info type);
consteval info underlying_type(info type);
consteval info unwrap_reference(info type);
consteval info unwrap_ref_decay(info type);

// tuple and variant queries (over the TU's std::tuple_size etc.)
consteval __kiln_size_t tuple_size(info type);
consteval info tuple_element(__kiln_size_t index, info type);
consteval __kiln_size_t variant_size(info type);
consteval info variant_alternative(__kiln_size_t index, info type);

// The range forms' kiln encoding (spec/reflection-traits.md §1.3): the
// range as a pointer to its first element and a count. The IS's
// reflection_range overloads below forward here.
consteval bool is_constructible_type(info type, const info* args, __kiln_size_t n);
consteval bool is_trivially_constructible_type(info type, const info* args, __kiln_size_t n);
consteval bool is_nothrow_constructible_type(info type, const info* args, __kiln_size_t n);
consteval bool is_invocable_type(info type, const info* args, __kiln_size_t n);
consteval bool is_invocable_r_type(info r, info type, const info* args, __kiln_size_t n);
consteval bool is_nothrow_invocable_type(info type, const info* args, __kiln_size_t n);
consteval bool is_nothrow_invocable_r_type(info r, info type, const info* args, __kiln_size_t n);
consteval info invoke_result(info type, const info* args, __kiln_size_t n);
consteval info common_type(const info* args, __kiln_size_t n);
consteval info common_reference(const info* args, __kiln_size_t n);
}  // namespace std::meta

// The IS's reflection_range overloads ([meta.reflection.traits]): templates
// copying the range into a constant-evaluation buffer and calling the
// (data, n) form. They live in a non-inline sub-namespace so they are not
// themselves taken for intrinsics, and are redeclared into std::meta.
namespace std::meta::__kiln_range {
template <class R>
consteval __kiln_size_t count(R&& r) {
  __kiln_size_t n = 0;
  for (info x : r) {
    (void)x;
    ++n;
  }
  return n;
}

#define KILN_META_RANGE(RET, NAME, LEAD, LEADARGS)          \
  template <class R = std::initializer_list<info>>          \
  consteval RET NAME(LEAD R&& range) {                      \
    __kiln_size_t n = count(range);                         \
    info* p = new info[n ? n : 1];                          \
    __kiln_size_t i = 0;                                    \
    for (info x : range) p[i++] = x;                        \
    RET v = ::std::meta::NAME(LEADARGS p, n);               \
    delete[] p;                                             \
    return v;                                               \
  }
#define KILN_COMMA ,
KILN_META_RANGE(bool, is_constructible_type, info type KILN_COMMA, type KILN_COMMA)
KILN_META_RANGE(bool, is_trivially_constructible_type, info type KILN_COMMA, type KILN_COMMA)
KILN_META_RANGE(bool, is_nothrow_constructible_type, info type KILN_COMMA, type KILN_COMMA)
KILN_META_RANGE(bool, is_invocable_type, info type KILN_COMMA, type KILN_COMMA)
KILN_META_RANGE(bool, is_nothrow_invocable_type, info type KILN_COMMA, type KILN_COMMA)
KILN_META_RANGE(bool, is_invocable_r_type, info r KILN_COMMA info type KILN_COMMA,
                r KILN_COMMA type KILN_COMMA)
KILN_META_RANGE(bool, is_nothrow_invocable_r_type, info r KILN_COMMA info type KILN_COMMA,
                r KILN_COMMA type KILN_COMMA)
KILN_META_RANGE(info, invoke_result, info type KILN_COMMA, type KILN_COMMA)
KILN_META_RANGE(info, common_type, , )
KILN_META_RANGE(info, common_reference, , )
#undef KILN_COMMA
#undef KILN_META_RANGE
}  // namespace std::meta::__kiln_range

namespace std::meta {
using __kiln_range::common_reference;
using __kiln_range::common_type;
using __kiln_range::invoke_result;
using __kiln_range::is_constructible_type;
using __kiln_range::is_invocable_r_type;
using __kiln_range::is_invocable_type;
using __kiln_range::is_nothrow_constructible_type;
using __kiln_range::is_nothrow_invocable_r_type;
using __kiln_range::is_nothrow_invocable_type;
using __kiln_range::is_trivially_constructible_type;
}  // namespace std::meta
