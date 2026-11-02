// [depr]-style -Wdeprecated-builtins (clang 22): `__is_trivially_relocatable`
// is deprecated in favour of the P2786 `__builtin_is_cpp_trivially_relocatable`;
// the trait keeps its clang 19 meaning. Clang 19 does not warn.
//@ expect: compile-ok
//@ strict-diags
//@ requires: clang>=22
//@ diag: 10:15 warning "builtin __is_trivially_relocatable is deprecated; use __builtin_is_cpp_trivially_relocatable instead"
//@ diag: 11:16 warning "builtin __is_trivially_relocatable is deprecated; use __builtin_is_cpp_trivially_relocatable instead"
struct E {};
static_assert(__is_trivially_relocatable(E));
static_assert(!__is_trivially_relocatable(int&));
static_assert(__builtin_is_cpp_trivially_relocatable(E));
