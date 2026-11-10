// [meta.reflection.layout] size_of, alignment_of, [temp.inst]/2,
// [expr.const] (P2996R13; spec/sema-templates.md §10.2, WP-1248):
// `size_of` and `alignment_of` of a class template specialization
// implicitly instantiate it from inside the constant evaluation. An error
// in that instantiation ends its context with the evaluation's call
// frames, innermost first, down to the outermost evaluation (the
// static_assert), as GCC 16.1's "in 'constexpr' expansion of" lines do
// (checked by hand).
//@ expect: compile-fail
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ oracle: gcc
//@ strict-diags
//@ diag: 25:35 error "field type 'Shape' is an abstract class"
//@ diag: 28:54 note "in instantiation of template class 'Box<Shape>' requested here"
//@ diag: 30:37 note "in call to 'sz(^^Box<Shape>)'"
//@ diag: 32:15 note "in call to 'walk_size()'"
//@ diag: 24:28 note "unimplemented pure virtual method 'area' in 'Shape'"
//@ diag: 26:35 error "field type 'Shape' is an abstract class"
//@ diag: 29:54 note "in instantiation of template class 'Bin<Shape>' requested here"
//@ diag: 31:38 note "in call to 'al(^^Bin<Shape>)'"
//@ diag: 33:15 note "in call to 'walk_align()'"
#include <meta>
struct Shape { virtual int area() = 0; };
template <class T> struct Box { T item; };
template <class T> struct Bin { T item; };

consteval std::size_t sz(std::meta::info t) { return std::meta::size_of(t); }
consteval std::size_t al(std::meta::info t) { return std::meta::alignment_of(t); }
consteval bool walk_size() { return sz(^^Box<Shape>) > 0; }
consteval bool walk_align() { return al(^^Bin<Shape>) > 0; }
static_assert(walk_size());
static_assert(walk_align());
