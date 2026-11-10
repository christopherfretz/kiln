// [meta.reflection.member.queries] members_of, [temp.inst]/2, [expr.const]
// (P2996R13; spec/sema-templates.md §10.2, WP-1248): `members_of` of a
// class template specialization implicitly instantiates it from inside
// the constant evaluation. An error in that instantiation ends the
// instantiation's context with the evaluation's call frames, innermost
// first, down to the outermost evaluation (the static_assert); a
// static_assert that is itself in an instantiation prints that
// instantiation's note after the frames (GCC 16.1 prints the same chain,
// checked by hand).
//@ expect: compile-fail
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ oracle: gcc
//@ strict-diags
//@ diag: 27:37 error "field type 'Shape' is an abstract class"
//@ diag: 31:10 note "in instantiation of template class 'Crate<Shape>' requested here"
//@ diag: 33:32 note "in call to 'count(^^Crate<Shape>)'"
//@ diag: 34:15 note "in call to 'walk()'"
//@ diag: 26:28 note "unimplemented pure virtual method 'area' in 'Shape'"
//@ diag: 28:36 error "field type 'Shape' is an abstract class"
//@ diag: 31:10 note "in instantiation of template class 'Tray<Shape>' requested here"
//@ diag: 37:53 note "in call to 'count(^^Tray<Shape>)'"
//@ diag: 38:50 note "in call to 'walk_t<Shape>()'"
//@ diag: 39:15 note "in instantiation of template class 'Holder<Shape>' requested here"
#include <meta>
struct Shape { virtual int area() = 0; };
template <class T> struct Crate { T item; };
template <class T> struct Tray { T item; };

consteval std::size_t count(std::meta::info t) {
  return std::meta::members_of(t, std::meta::access_context::unchecked()).size();
}
consteval bool walk() { return count(^^Crate<Shape>) > 0; }
static_assert(walk());

// The evaluation runs in an instantiation of `Holder<Shape>`.
template <class T> consteval bool walk_t() { return count(^^Tray<T>) > 0; }
template <class T> struct Holder { static_assert(walk_t<T>()); };
Holder<Shape> h;
