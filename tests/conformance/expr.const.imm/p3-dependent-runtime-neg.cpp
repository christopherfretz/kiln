// [expr.const.imm]/1.2, /3 (P4101R1): only a subexpression of a
// manifestly constant-evaluated expression is in an immediate function
// context. Instantiated, a call returning a consteval-only value in a
// run-time initializer is still an error (a reflection, a class holding
// one, and a default member initializer used by a run-time object), while
// the same call in a template argument of the same instantiation is
// allowed (p1-constant-contexts-dependent-run.cpp). Compared with GCC
// 16.1 (WP-1328).
//@ expect: compile-fail
//@ oracle: gcc
//@ args: -freflection
//@ skip(clang): clang 19/22 do not implement P2996 reflection
//@ strict-diags
//@ diag: 26:47 error "consteval-only value outside an immediate function context"
//@ diag: 31:9 note "in instantiation of function template specialization 'copy_info<int>' requested here"
//@ diag: 27:45 error "consteval-only value outside an immediate function context"
//@ diag: 32:9 note "in instantiation of function template specialization 'copy_box<int>' requested here"
//@ diag: 29:37 error "consteval-only value outside an immediate function context"
//@ diag: 33:9 note "in instantiation of function template specialization 'dmi<int>' requested here"
using info = decltype(^^int);
struct Box { info r; int n; };
template <class T> consteval info key() { return ^^T; }
template <class T> consteval Box box() { return {^^T, 1}; }
template <info H> struct C { static constexpr int v = 1; };

template <class T> int copy_info() { info x = key<T>(); return C<key<T>()>::v; }
template <class T> int copy_box() { Box b = box<T>(); return C<box<T>().r>::v; }
template <class T> struct S { info r = key<T>(); };
template <class T> int dmi() { S<T> s; (void)s; return C<S<T>{}.r>::v; }

int a = copy_info<int>();
int b = copy_box<int>();
int c = dmi<int>();
