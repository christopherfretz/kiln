// [meta.reflection.substitute], [temp.inst]/2, [expr.const] (P2996R13;
// spec/sema-templates.md §10.2, WP-1248): `substitute` forms a class
// template specialization without instantiating it; a query of the
// result that needs the class complete (`is_complete_type`, `size_of`)
// instantiates it from inside the constant evaluation. An error in that
// instantiation ends its context with the evaluation's call frames,
// innermost first, down to the outermost evaluation (the static_assert),
// as GCC 16.1's "in 'constexpr' expansion of" lines do (checked by hand).
//@ expect: compile-fail
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ oracle: gcc
//@ strict-diags
//@ diag: 24:35 error "field type 'Shape' is an abstract class"
//@ diag: 30:56 note "in instantiation of template class 'Jar<Shape>' requested here"
//@ diag: 31:32 note "in call to 'complete(^^Jar)'"
//@ diag: 32:15 note "in call to 'walk()'"
//@ diag: 23:28 note "unimplemented pure virtual method 'area' in 'Shape'"
//@ diag: 25:35 error "field type 'Shape' is an abstract class"
//@ diag: 34:60 note "in instantiation of template class 'Urn<Shape>' requested here"
//@ diag: 35:15 note "in call to 'bytes(^^Urn)'"
#include <meta>
struct Shape { virtual int area() = 0; };
template <class T> struct Jar { T item; };
template <class T> struct Urn { T item; };

consteval std::meta::info make(std::meta::info tmpl) {
  return std::meta::substitute(tmpl, {^^Shape});
}
consteval bool complete(std::meta::info tmpl) { return std::meta::is_complete_type(make(tmpl)); }
consteval bool walk() { return complete(^^Jar); }
static_assert(walk());

consteval std::size_t bytes(std::meta::info tmpl) { return std::meta::size_of(make(tmpl)); }
static_assert(bytes(^^Urn) > 0);
