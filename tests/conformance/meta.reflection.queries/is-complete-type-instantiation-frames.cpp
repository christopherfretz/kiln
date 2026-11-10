// [meta.reflection.queries] is_complete_type, [temp.inst]/2, [expr.const]
// (P2996R13; spec/sema-templates.md §10.2, WP-1248): `is_complete_type`
// of a class template specialization implicitly instantiates it from
// inside the constant evaluation. An error in that instantiation ends its
// context with the evaluation's call frames, innermost first, in the
// "in call to" form of a failing evaluation's backtrace, down to the
// outermost evaluation (the static_assert), as GCC 16.1's "in 'constexpr'
// expansion of" lines do (checked by hand). The frames follow
// -fconstexpr-backtrace-limit's elision.
//@ expect: compile-fail
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ oracle: gcc
//@ strict-diags
//@ diag: 37:37 error "field type 'Shape' is an abstract class"
//@ diag: 40:50 note "in instantiation of template class 'Crate<Shape>' requested here"
//@ diag: 41:32 note "in call to 'probe(^^Crate<Shape>)'"
//@ diag: 42:15 note "in call to 'walk()'"
//@ diag: 35:28 note "unimplemented pure virtual method 'area' in 'Shape'"
//@ diag: 37:37 error "field type 'Pane' is an abstract class"
//@ diag: 40:50 note "in instantiation of template class 'Crate<Pane>' requested here"
//@ diag: 47:12 note "in call to 'probe(^^Crate<Pane>)'"
//@ diag: 49:12 note "in call to 'deep<0>()'"
//@ diag: 49:12 note "in call to 'deep<1>()'"
//@ diag: 49:12 note "in call to 'deep<2>()'"
//@ diag: 49:12 note "in call to 'deep<3>()'"
//@ diag: 49:12 note "(skipping 2 calls in backtrace; use -fconstexpr-backtrace-limit=0 to see all)"
//@ diag: 49:12 note "in call to 'deep<6>()'"
//@ diag: 49:12 note "in call to 'deep<7>()'"
//@ diag: 49:12 note "in call to 'deep<8>()'"
//@ diag: 49:12 note "in call to 'deep<9>()'"
//@ diag: 51:15 note "in call to 'deep<10>()'"
//@ diag: 36:28 note "unimplemented pure virtual method 'draw' in 'Pane'"
#include <meta>
struct Shape { virtual int area() = 0; };
struct Pane { virtual void draw() = 0; };
template <class T> struct Crate { T item; };

// The reported shape: a walker calls a probe that asks the question.
consteval bool probe(std::meta::info t) { return std::meta::is_complete_type(t); }
consteval bool walk() { return probe(^^Crate<Shape>); }
static_assert(walk());

// Twelve frames deep: the default limit (10) elides the middle two.
template <int N> consteval bool deep() {
  if constexpr (N == 0)
    return probe(^^Crate<Pane>);
  else
    return deep<N - 1>();
}
static_assert(deep<10>());
