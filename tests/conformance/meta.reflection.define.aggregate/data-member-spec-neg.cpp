// [meta.reflection.define.aggregate]/5 (P2996R13; GCC 16.1's order and
// rules, spec/reflection.md §6.1): data_member_spec throws a
// std::meta::exception (uncaught here, so not a constant expression)
// for, in order: a non-type; a type that is not an object or reference
// type; a name that is not an identifier, or a keyword; neither a name
// nor a bit width; both an alignment and a bit width; an alignment that
// is not a power of two or is less than the type's; a bit width with a
// non-integral, non-enumeration type, with no_unique_address, negative,
// or 0 with a name; an unnamed bit-field of cv-qualified type.
//@ expect: compile-fail
//@ args: -ferror-limit=0 -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
//@ diag: 65:15 error "static assertion expression is not an integral constant expression"
//@ diag: 65:35 note "'^^var' does not reflect a type"
//@ diag: 66:15 error "static assertion expression is not an integral constant expression"
//@ diag: 66:35 note "'void' is not an object or reference type"
//@ diag: 67:15 error "static assertion expression is not an integral constant expression"
//@ diag: 67:35 note "'int ()' is not an object or reference type"
//@ diag: 68:15 error "static assertion expression is not an integral constant expression"
//@ diag: 68:35 note "'1a' is not a valid identifier"
//@ diag: 69:15 error "static assertion expression is not an integral constant expression"
//@ diag: 69:35 note "'' is not a valid identifier"
//@ diag: 70:15 error "static assertion expression is not an integral constant expression"
//@ diag: 70:35 note "'a-b' is not a valid identifier"
//@ diag: 71:15 error "static assertion expression is not an integral constant expression"
//@ diag: 71:35 note "'while' is a keyword, not an identifier"
//@ diag: 72:15 error "static assertion expression is not an integral constant expression"
//@ diag: 72:35 note "'char8_t' is a keyword, not an identifier"
//@ diag: 73:15 error "static assertion expression is not an integral constant expression"
//@ diag: 73:35 note "neither 'name' nor 'bit_width' specified"
//@ diag: 74:15 error "static assertion expression is not an integral constant expression"
//@ diag: 74:35 note "neither 'name' nor 'bit_width' specified"
//@ diag: 75:15 error "static assertion expression is not an integral constant expression"
//@ diag: 75:35 note "both 'alignment' and 'bit_width' specified"
//@ diag: 76:15 error "static assertion expression is not an integral constant expression"
//@ diag: 76:35 note "'alignment' 3 is not a positive power of two"
//@ diag: 77:15 error "static assertion expression is not an integral constant expression"
//@ diag: 77:35 note "'alignment' 0 is not a positive power of two"
//@ diag: 78:15 error "static assertion expression is not an integral constant expression"
//@ diag: 78:35 note "'alignment' -8 is not a positive power of two"
//@ diag: 79:15 error "static assertion expression is not an integral constant expression"
//@ diag: 79:35 note "'alignment' 2 is smaller than the alignment 4 of 'int'"
//@ diag: 80:15 error "static assertion expression is not an integral constant expression"
//@ diag: 80:35 note "'bit_width' specified with non-integral and non-enumeration type 'float'"
//@ diag: 81:15 error "static assertion expression is not an integral constant expression"
//@ diag: 81:35 note "'bit_width' specified with non-integral and non-enumeration type 'int *'"
//@ diag: 82:15 error "static assertion expression is not an integral constant expression"
//@ diag: 82:35 note "'bit_width' specified with 'no_unique_address'"
//@ diag: 83:15 error "static assertion expression is not an integral constant expression"
//@ diag: 83:35 note "'bit_width' -1 is negative"
//@ diag: 84:15 error "static assertion expression is not an integral constant expression"
//@ diag: 84:35 note "'bit_width' 0 specified with a 'name'"
//@ diag: 85:15 error "static assertion expression is not an integral constant expression"
//@ diag: 85:35 note "unnamed bit-field of cv-qualified type 'const int'"
//@ diag: 86:15 error "static assertion expression is not an integral constant expression"
//@ diag: 86:35 note "unnamed bit-field of cv-qualified type 'volatile int'"
//@ diag: 88:15 error "static assertion expression is not an integral constant expression"
//@ diag: 88:35 note "'void' is not an object or reference type"
//@ diag: 89:15 error "static assertion expression is not an integral constant expression"
//@ diag: 89:35 note "'while' is a keyword, not an identifier"
#include "kiln_meta.h"
using namespace std::meta;
int var;
static_assert(is_data_member_spec(data_member_spec(^^var, {.name = "a"})));
static_assert(is_data_member_spec(data_member_spec(^^void, {.name = "a"})));
static_assert(is_data_member_spec(data_member_spec(^^int(), {.name = "a"})));
static_assert(is_data_member_spec(data_member_spec(^^int, {.name = "1a"})));
static_assert(is_data_member_spec(data_member_spec(^^int, {.name = ""})));
static_assert(is_data_member_spec(data_member_spec(^^int, {.name = "a-b"})));
static_assert(is_data_member_spec(data_member_spec(^^int, {.name = "while"})));
static_assert(is_data_member_spec(data_member_spec(^^int, {.name = "char8_t"})));
static_assert(is_data_member_spec(data_member_spec(^^int, {})));
static_assert(is_data_member_spec(data_member_spec(^^int, {.alignment = 4})));
static_assert(is_data_member_spec(data_member_spec(^^int, {.name = "a", .alignment = 4, .bit_width = 3})));
static_assert(is_data_member_spec(data_member_spec(^^int, {.name = "a", .alignment = 3})));
static_assert(is_data_member_spec(data_member_spec(^^int, {.name = "a", .alignment = 0})));
static_assert(is_data_member_spec(data_member_spec(^^int, {.name = "a", .alignment = -8})));
static_assert(is_data_member_spec(data_member_spec(^^int, {.name = "a", .alignment = 2})));
static_assert(is_data_member_spec(data_member_spec(^^float, {.name = "a", .bit_width = 3})));
static_assert(is_data_member_spec(data_member_spec(^^int*, {.name = "a", .bit_width = 3})));
static_assert(is_data_member_spec(data_member_spec(^^int, {.name = "a", .bit_width = 3, .no_unique_address = true})));
static_assert(is_data_member_spec(data_member_spec(^^int, {.name = "a", .bit_width = -1})));
static_assert(is_data_member_spec(data_member_spec(^^int, {.name = "a", .bit_width = 0})));
static_assert(is_data_member_spec(data_member_spec(^^const int, {.bit_width = 3})));
static_assert(is_data_member_spec(data_member_spec(^^volatile int, {.bit_width = 0})));
// The first failing rule is reported.
static_assert(is_data_member_spec(data_member_spec(^^void, {.name = "while", .alignment = 3})));
static_assert(is_data_member_spec(data_member_spec(^^int, {.name = "while", .alignment = 3})));
