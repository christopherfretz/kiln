// [meta.reflection.queries] is_complete_type, [dcl.constexpr]/6,
// [temp.inst]/2 (P2996R13; spec/sema.md §5.28 "Never constant",
// spec/sema-templates.md §10.2, WP-1258): under -Winvalid-constexpr the
// definition-time never-constant check evaluates a consteval function's
// body, and a reflection query there may instantiate a specialization.
// An error in that instantiation names only the program's frames: the
// check's own evaluation of the function is no call of the program's,
// so no "in call to" note follows the instantiation frame (the
// diagnostic is the one an instantiation outside any evaluation prints).
// GCC 16.1 has no such check and diagnoses the same instantiation.
//@ expect: compile-fail
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain} -Winvalid-constexpr
//@ skip(clang): clang does not implement P2996 reflection
//@ oracle: gcc
//@ strict-diags
//@ diag: 25:37 error "field type 'Shape' is an abstract class"
//@ diag: 29:12 note "in instantiation of template class 'Crate<Shape>' requested here"
//@ diag: 23:28 note "unimplemented pure virtual method 'area' in 'Shape'"
//@ diag: 25:37 error "field type 'Pane' is an abstract class"
//@ diag: 35:12 note "in instantiation of template class 'Crate<Pane>' requested here"
//@ diag: 24:28 note "unimplemented pure virtual method 'draw' in 'Pane'"
#include <meta>
struct Shape { virtual int area() = 0; };
struct Pane { virtual void draw() = 0; };
template <class T> struct Crate { T item; };

// A function without parameters: the check calls it.
consteval bool probe() {
  bool b = std::meta::is_complete_type(^^Crate<Shape>);
  return b;
}

// One with parameters: the check walks its body.
consteval bool probe_at(int k) {
  bool b = std::meta::is_complete_type(^^Crate<Pane>);
  return b && k;
}
// The calls the program makes: the class is diagnosed once, and both
// compilers answer that it is complete.
static_assert(probe());
static_assert(probe_at(1));
