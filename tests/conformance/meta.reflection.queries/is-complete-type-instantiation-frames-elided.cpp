// [meta.reflection.queries] is_complete_type, [temp.inst]/2, [expr.const]
// (P2996R13; spec/sema-templates.md §10.2, WP-1258): when
// -ftemplate-backtrace-limit elides the instantiation frame a reflection
// query's call frames follow, the call frames stay: they follow clang
// 22's "(skipping N contexts in backtrace; ...)" note, in their place
// innermost first, before the next frame shown. Only the instantiation
// frames are elided (the "in call to" frames follow
// -fconstexpr-backtrace-limit alone).
//@ expect: compile-fail
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain} -ftemplate-backtrace-limit=2
//@ skip(clang): clang does not implement P2996 reflection
//@ oracle: gcc
//@ strict-diags
//@ diag: 24:37 error "field type 'Shape' is an abstract class"
//@ diag: 25:43 note "in instantiation of template class 'Crate<Shape>' requested here"
//@ diag: 26:41 note "(skipping 3 contexts in backtrace; use -ftemplate-backtrace-limit=0 to see all)"
//@ diag: 29:49 note "in call to 'inner(1)'"
//@ diag: 31:50 note "in instantiation of template class 'Shell<char>' requested here"
//@ diag: 32:38 note "in call to 'probe(^^Shell<char>)'"
//@ diag: 33:15 note "in call to 'outer(7)'"
//@ diag: 23:28 note "unimplemented pure virtual method 'area' in 'Shape'"
#include <meta>
struct Shape { virtual int area() = 0; };
template <class T> struct Crate { T item; };
template <class T> struct Wrap { Crate<T> c; };
template <class T> struct Box { Wrap<T> w; };

consteval bool inner(int k) { return std::meta::is_complete_type(^^Box<Shape>); }
template <class T> struct Outer { static_assert(inner(sizeof(T))); };
template <class T> struct Shell { Outer<T> o; };
consteval bool probe(std::meta::info t) { return std::meta::is_complete_type(t); }
consteval bool outer(int j) { return probe(^^Shell<char>); }
static_assert(outer(7));
