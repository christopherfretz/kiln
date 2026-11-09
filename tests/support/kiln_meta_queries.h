// The [meta.reflection.queries] family and P3096's parameter queries
// (WP-476; spec/reflection-queries.md), over kiln_meta.h's `info`.
// Kiln-internal: the names and signatures of GCC 16.1's <meta>, which
// WP-484 replaces with the real header. The functions with no body are
// consteval intrinsics the Oven dispatches by qualified name and
// signature (spec/reflection.md §3).
#pragma once

#include "kiln_meta.h"

namespace std::meta {
// GCC 16.1's numbering ([meta.reflection.operators]).
enum class operators {
  op_new = 1, op_delete, op_array_new, op_array_delete, op_co_await,
  op_parentheses, op_square_brackets, op_arrow, op_arrow_star, op_tilde,
  op_exclamation, op_plus, op_minus, op_star, op_slash, op_percent, op_caret,
  op_ampersand, op_equals, op_pipe, op_plus_equals, op_minus_equals,
  op_star_equals, op_slash_equals, op_percent_equals, op_caret_equals,
  op_ampersand_equals, op_pipe_equals, op_equals_equals, op_exclamation_equals,
  op_less, op_greater, op_less_equals, op_greater_equals, op_spaceship,
  op_ampersand_ampersand, op_pipe_pipe, op_less_less, op_greater_greater,
  op_less_less_equals, op_greater_greater_equals, op_plus_plus, op_minus_minus,
  op_comma,
};
using enum operators;

consteval std::string_view symbol_of(operators op);
consteval std::u8string_view u8symbol_of(operators op);

consteval info object_of(info r);
consteval info constant_of(info r);
consteval info parent_of(info r);
consteval info dealias(info r);
consteval bool has_template_arguments(info r);
consteval operators operator_of(info r);
consteval info variable_of(info r);
consteval info return_type_of(info r);
consteval bool is_function_parameter(info r);
consteval bool is_explicit_object_parameter(info r);
consteval bool has_default_argument(info r);
consteval bool is_vararg_function(info r);
consteval bool is_public(info r);
consteval bool is_protected(info r);
consteval bool is_private(info r);
consteval bool is_virtual(info r);
consteval bool is_pure_virtual(info r);
consteval bool is_override(info r);
consteval bool is_final(info r);
consteval bool is_deleted(info r);
consteval bool is_defaulted(info r);
consteval bool is_user_provided(info r);
consteval bool is_user_declared(info r);
consteval bool is_explicit(info r);
consteval bool is_noexcept(info r);
consteval bool is_bit_field(info r);
consteval bool is_enumerator(info r);
consteval bool is_const(info r);
consteval bool is_volatile(info r);
consteval bool is_mutable_member(info r);
consteval bool is_lvalue_reference_qualified(info r);
consteval bool is_rvalue_reference_qualified(info r);
consteval bool has_static_storage_duration(info r);
consteval bool has_thread_storage_duration(info r);
consteval bool has_automatic_storage_duration(info r);
consteval bool has_internal_linkage(info r);
consteval bool has_module_linkage(info r);
consteval bool has_external_linkage(info r);
consteval bool has_c_language_linkage(info r);
consteval bool has_linkage(info r);
consteval bool is_complete_type(info r);
consteval bool is_enumerable_type(info r);
consteval bool is_variable(info r);
consteval bool is_type(info r);
consteval bool is_namespace(info r);
consteval bool is_type_alias(info r);
consteval bool is_namespace_alias(info r);
consteval bool is_function(info r);
consteval bool is_conversion_function(info r);
consteval bool is_operator_function(info r);
consteval bool is_literal_operator(info r);
consteval bool is_special_member_function(info r);
consteval bool is_constructor(info r);
consteval bool is_default_constructor(info r);
consteval bool is_copy_constructor(info r);
consteval bool is_move_constructor(info r);
consteval bool is_assignment(info r);
consteval bool is_copy_assignment(info r);
consteval bool is_move_assignment(info r);
consteval bool is_destructor(info r);
consteval bool is_template(info r);
consteval bool is_function_template(info r);
consteval bool is_variable_template(info r);
consteval bool is_class_template(info r);
consteval bool is_alias_template(info r);
consteval bool is_conversion_function_template(info r);
consteval bool is_operator_function_template(info r);
consteval bool is_literal_operator_template(info r);
consteval bool is_constructor_template(info r);
consteval bool is_concept(info r);
consteval bool is_structured_binding(info r);
consteval bool is_value(info r);
consteval bool is_object(info r);
consteval bool is_class_member(info r);
consteval bool is_namespace_member(info r);
consteval bool is_nonstatic_data_member(info r);
consteval bool is_static_member(info r);
consteval bool is_base(info r);
consteval bool has_default_member_initializer(info r);
consteval bool has_parent(info r);

// Other families' predicates this WP answers.
consteval bool is_annotation(info r);
consteval bool is_data_member_spec(info r);
}  // namespace std::meta
