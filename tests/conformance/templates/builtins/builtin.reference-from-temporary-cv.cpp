// [meta.unary.prop] reference_constructs_from_temporary /
// reference_converts_from_temporary, [expr.type]/2: VAL<U> for a
// non-reference U is a prvalue, and a prvalue of a non-class, non-array
// type has its cv-qualifiers dropped, so `const int` initializes `int&&`
// through a temporary (a class prvalue keeps its `const`). clang 19 keeps
// the cv-qualifiers (kiln does under -fclang-compat=19).
//@ expect: compile-ok
//@ requires: clang>=22
static_assert(__reference_constructs_from_temporary(int&&, const int));
static_assert(__reference_converts_from_temporary(int&&, const int));
static_assert(__reference_constructs_from_temporary(const int&, const int));
static_assert(!__reference_constructs_from_temporary(int&, const int));
struct S {};
static_assert(!__reference_constructs_from_temporary(S&&, const S));
static_assert(__reference_constructs_from_temporary(const S&&, const S));
