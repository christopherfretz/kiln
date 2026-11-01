// [class.prop]/2, [meta.unary.prop] (C++26, P2786R13): trivially relocatable
// classes, queried by `__builtin_is_cpp_trivially_relocatable`. clang 22
// defines `__cpp_trivial_relocatability` and the builtin but no longer
// accepts the `trivially_relocatable_if_eligible` specifier (P2786 was
// removed from the working draft in 2025), so only the builtin is probed.
// WP-185 feature probe; kiln implements the builtin since WP-263
// (tests/conformance/ext.traits/is-cpp-trivially-relocatable.cpp).
//@ expect: compile-ok
//@ strict-diags
//@ xfail(clang): clang 19 lacks P2786 (clang 22 accepts the builtin)
struct S { int x; };
struct T { T(T&&); };
static_assert(__builtin_is_cpp_trivially_relocatable(S));
static_assert(!__builtin_is_cpp_trivially_relocatable(T));
