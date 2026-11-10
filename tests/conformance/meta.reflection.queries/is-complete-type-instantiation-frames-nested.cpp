// [meta.reflection.queries] is_complete_type, [temp.inst]/2, [expr.const]
// (P2996R13; spec/sema-templates.md §10.2, WP-1258): an instantiation a
// reflection query requires from a constant evaluation nested in another
// (a static_assert in a class template specialization the outer
// evaluation's query instantiates) ends each evaluation's part of the
// context with that evaluation's call frames: the nested one's after the
// innermost instantiation, the outer one's after the specialization its
// query required, down to the outermost static_assert, as GCC 16.1's
// "in 'constexpr' expansion of" lines name both evaluations (checked by
// hand).
//@ expect: compile-fail
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ oracle: gcc
//@ strict-diags
//@ diag: 25:37 error "field type 'Shape' is an abstract class"
//@ diag: 27:38 note "in instantiation of template class 'Crate<Shape>' requested here"
//@ diag: 28:49 note "in call to 'inner(1)'"
//@ diag: 29:50 note "in instantiation of template class 'Outer<char>' requested here"
//@ diag: 30:38 note "in call to 'probe(^^Outer<char>)'"
//@ diag: 31:15 note "in call to 'outer(7)'"
//@ diag: 24:28 note "unimplemented pure virtual method 'area' in 'Shape'"
#include <meta>
struct Shape { virtual int area() = 0; };
template <class T> struct Crate { T item; };

consteval bool inner(int k) { return std::meta::is_complete_type(^^Crate<Shape>); }
template <class T> struct Outer { static_assert(inner(sizeof(T))); };
consteval bool probe(std::meta::info t) { return std::meta::is_complete_type(t); }
consteval bool outer(int j) { return probe(^^Outer<char>); }
static_assert(outer(7));
