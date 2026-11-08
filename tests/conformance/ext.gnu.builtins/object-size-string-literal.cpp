// [expr.const]/10 __builtin_object_size of a pointer into a string
// literal (clang ExprConstant `determineEndOffset` / `getObjectType`):
// only a variable, a compound literal or a dynamic allocation gives the
// whole object a type, so types 0 and 2 are not a constant; types 1 and 3
// use the literal's array designator. Verified against clang 19 and 22.
// spec/oven.md "library builtins", WP-808.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 14:15 error "static assertion expression is not an integral constant expression"
//@ diag: 15:15 error "static assertion expression is not an integral constant expression"
static_assert(__builtin_object_size("abc", 1) == 4);
static_assert(__builtin_object_size(&"abc"[1], 3) == 3);
static_assert(__builtin_dynamic_object_size(&"abcdef"[2], 1) == 5);
static_assert(__builtin_object_size("abc", 0) == 4);
static_assert(__builtin_object_size("abc", 2) == 4);
