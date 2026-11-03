// [dcl.attr.grammar]/5: nonnull and returns_nonnull on a template parameter type
// are accepted from clang 22 on (nonnull-attribute-dependent.cpp: clang 19
// warns) (WP-421).
//@ expect: compile-ok
//@ strict-diags
//@ requires: clang>=22
template <class T> void a13(T) __attribute__((nonnull(1)));
template <class T> void b13(T) __attribute__((nonnull));
template <class T> T a15() __attribute__((returns_nonnull));
template <class T> void a16(__attribute__((nonnull)) T);
template <class T> void a17(T&) __attribute__((nonnull(1)));
template <class T> void a18(T*) __attribute__((nonnull(1)));
template <class T> struct Q { void m(T) __attribute__((nonnull(2))); };
