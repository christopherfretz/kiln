// [dcl.attr.grammar]/5: GNU nonnull and returns_nonnull (clang handleNonNullAttr,
// handleReturnsNonNullAttr): on a function or parameter; each index an integer
// constant naming a parameter (one-based, not the implicit object parameter, any
// past the last of a variadic function) of pointer type; with no index, a
// function with a pointer parameter; returns_nonnull on a function returning a
// pointer (WP-421).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 21:23 warning "'nonnull' attribute only applies to functions, methods, and parameters [-Wignored-attributes]"
//@ diag: 22:22 warning "'returns_nonnull' attribute only applies to Objective-C methods and functions [-Wignored-attributes]"
//@ diag: 23:25 warning "'returns_nonnull' attribute only applies to return values that are pointers [-Wignored-attributes]"
//@ diag: 24:29 warning "'nonnull' attribute applied to function with no pointer arguments [-Wignored-attributes]"
//@ diag: 25:35 warning "'nonnull' attribute only applies to pointer arguments [-Wignored-attributes]"
//@ diag: 26:30 error "'nonnull' attribute parameter 1 is out of bounds"
//@ diag: 27:30 error "'nonnull' attribute parameter 1 is out of bounds"
//@ diag: 28:24 warning "'nonnull' attribute only applies to pointer arguments [-Wignored-attributes]"
//@ diag: 30:30 error "'nonnull' attribute requires parameter 1 to be an integer constant"
//@ diag: 31:40 error "'nonnull' attribute is invalid for the implicit this argument"
//@ diag: 33:30 warning "'nonnull' attribute applied to function with no pointer arguments [-Wignored-attributes]"
//@ diag: 38:49 error "'nonnull' attribute requires parameter 1 to be an integer constant"
int *v __attribute__((nonnull));
int w __attribute__((returns_nonnull));
int r1() __attribute__((returns_nonnull));
void a1(int) __attribute__((nonnull));
void a2(int, int*) __attribute__((nonnull(1)));
void a3(int*) __attribute__((nonnull(2)));
void a4(int*) __attribute__((nonnull(0)));
void a5(__attribute__((nonnull)) int);
int n;
void a6(int*) __attribute__((nonnull(n)));
struct S { void m(int*) __attribute__((nonnull(1))); };
void a7(int*, ...) __attribute__((nonnull(1, 3)));
void a8(int&) __attribute__((nonnull));
[[gnu::nonnull]] void a9(int*);
[[gnu::returns_nonnull]] int *a10();
void a11(int *) __attribute__((nonnull()));
void a12(int, ...) __attribute__((nonnull));
template <int N> void a14(int *) __attribute__((nonnull(N)));
