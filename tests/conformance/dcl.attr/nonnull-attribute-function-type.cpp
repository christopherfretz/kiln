// [dcl.attr.grammar]/5: GNU nonnull on a typedef, alias, variable,
// member or reference whose type is a prototyped function or a pointer or
// reference to one (clang's HasFunctionProto subject): kept, its indices
// checked against that prototype as on a function; a pointer to pointer to
// function or a non-function type is not a subject. Found running GCC's C++
// testsuite (c-c++-common/builtin-has-attribute-2). WP-1675.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 22:24 warning "'nonnull' attribute only applies to functions, methods, and parameters [-Wignored-attributes]"
//@ diag: 23:24 warning "'nonnull' attribute only applies to functions, methods, and parameters [-Wignored-attributes]"
//@ diag: 24:24 error "'nonnull' attribute parameter 1 is out of bounds"
//@ diag: 25:24 warning "'nonnull' attribute only applies to pointer arguments [-Wignored-attributes]"
//@ diag: 26:24 warning "'nonnull' attribute applied to function with no pointer arguments [-Wignored-attributes]"
typedef void F(void *, void *);
typedef __attribute__((nonnull)) F F1;
typedef __attribute__((nonnull(2))) void (*FP)(void *, void *);
typedef void (*FP2)(void *, void *) __attribute__((nonnull(1)));
using U __attribute__((nonnull(1))) = void(void *);
__attribute__((nonnull)) void (*vp)(void *);
struct S { __attribute__((nonnull(1))) void (*fp)(void *); };
__attribute__((nonnull)) void (&rf)(void *) = *vp;
typedef __attribute__((nonnull)) int I;
typedef __attribute__((nonnull)) void (**PP)(void *);
typedef __attribute__((nonnull(3))) void F3(void *, void *);
typedef __attribute__((nonnull(1))) void F4(int, void *);
typedef __attribute__((nonnull)) void NP(int);
