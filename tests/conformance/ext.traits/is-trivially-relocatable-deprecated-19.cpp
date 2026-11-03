// [depr]-style -Wdeprecated-builtins: clang 19 does not deprecate
// `__is_trivially_relocatable` (clang 22 does, in favour of the P2786
// `__builtin_is_cpp_trivially_relocatable`, which clang 19 lacks;
// `is-trivially-relocatable-deprecated.cpp`), so at -fclang-compat=19 neither
// the written trait nor its instantiation warns.
//@ expect: compile-ok
//@ strict-diags
//@ requires: clang<22
struct E {};
static_assert(__is_trivially_relocatable(E));
static_assert(!__is_trivially_relocatable(int&));
template <class T> constexpr bool r = __is_trivially_relocatable(T);
static_assert(r<E>);
