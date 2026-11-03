// [cpp.cond]/6, Table 22 (C++26, P2795R5): `__has_cpp_attribute
// (indeterminate)` is 202403L.
//@ expect: compile-ok
//@ strict-diags
//@ requires: clang>=23
static_assert(__has_cpp_attribute(indeterminate) == 202403L);
#if __has_cpp_attribute(indeterminate) != 202403L
#error "indeterminate"
#endif
