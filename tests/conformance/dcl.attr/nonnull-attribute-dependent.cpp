// [dcl.attr.grammar]/5: clang 19 treats a template parameter type as no pointer
// for nonnull and returns_nonnull (clang 22 accepts it:
// nonnull-attribute-dependent-clang22.cpp) (WP-421).
//@ expect: compile-ok
//@ strict-diags
//@ requires: clang<22
//@ diag: 12:47 warning "'nonnull' attribute only applies to pointer arguments [-Wignored-attributes]"
//@ diag: 14:43 warning "'returns_nonnull' attribute only applies to return values that are pointers [-Wignored-attributes]"
//@ diag: 15:44 warning "'nonnull' attribute only applies to pointer arguments [-Wignored-attributes]"
//@ diag: 16:48 warning "'nonnull' attribute only applies to pointer arguments [-Wignored-attributes]"
//@ diag: 18:56 warning "'nonnull' attribute only applies to pointer arguments [-Wignored-attributes]"
template <class T> void a13(T) __attribute__((nonnull(1)));
template <class T> void b13(T) __attribute__((nonnull));
template <class T> T a15() __attribute__((returns_nonnull));
template <class T> void a16(__attribute__((nonnull)) T);
template <class T> void a17(T&) __attribute__((nonnull(1)));
template <class T> void a18(T*) __attribute__((nonnull(1)));
template <class T> struct Q { void m(T) __attribute__((nonnull(2))); };
