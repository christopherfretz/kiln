// [depr]-style -Wdeprecated-builtins (clang 22): `__is_trivially_relocatable`
// is deprecated in favour of the P2786 `__builtin_is_cpp_trivially_relocatable`;
// the trait keeps its clang 19 meaning. The warning is issued where the trait
// is written and again by each instantiation that rebuilds it. Clang 19 does
// not warn (`is-trivially-relocatable-deprecated-19.cpp`).
//@ expect: compile-ok
//@ strict-diags
//@ requires: clang>=22
//@ diag: 15:15 warning "builtin __is_trivially_relocatable is deprecated; use __builtin_is_cpp_trivially_relocatable instead"
//@ diag: 16:16 warning "builtin __is_trivially_relocatable is deprecated; use __builtin_is_cpp_trivially_relocatable instead"
//@ diag: 18:39 warning "builtin __is_trivially_relocatable is deprecated; use __builtin_is_cpp_trivially_relocatable instead"
//@ diag: 18:39 warning "builtin __is_trivially_relocatable is deprecated; use __builtin_is_cpp_trivially_relocatable instead"
//@ diag: 19:15 note "in instantiation of variable template specialization 'r<E>' requested here"
struct E {};
static_assert(__is_trivially_relocatable(E));
static_assert(!__is_trivially_relocatable(int&));
static_assert(__builtin_is_cpp_trivially_relocatable(E));
template <class T> constexpr bool r = __is_trivially_relocatable(T);
static_assert(r<E>);
