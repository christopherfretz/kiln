// [meta.reflection.queries] is_complete_type, [temp.inst]/2 (P2996R13;
// spec/reflection-repr.md §5, spec/sema-templates.md §10.2, WP-1258): a
// reflection argument in an "in call to" note of an instantiation's
// context names a member of a class template specialization, or a type
// that is one, with the specialization's arguments at every level of its
// scope (`^^S<int>::In`, `^^const S<int>::In *`, `^^ns::R<char, 3>::Q::y`,
// `^^S<int>::f`), as clang's `printQualifiedName` names a declaration in a
// diagnostic.
//@ expect: compile-fail
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ oracle: gcc
//@ strict-diags
//@ diag: 20:37 error "field type 'Shape' is an abstract class"
//@ diag: 25:10 note "in instantiation of template class 'Crate<Shape>' requested here"
//@ diag: 27:15 note "in call to 'probe(^^S<int>::In, ^^const S<int>::In *, ^^ns::R<char, 3>::Q::y, ^^S<int>::f)'"
//@ diag: 19:28 note "unimplemented pure virtual method 'area' in 'Shape'"
#include <meta>
struct Shape { virtual int area() = 0; };
template <class T> struct Crate { T item; };
template <class T> struct S { struct In {}; void f(); };
namespace ns { template <class T, int N> struct R { struct Q { static inline int y; }; }; }

consteval bool probe(std::meta::info, std::meta::info, std::meta::info, std::meta::info) {
  return std::meta::is_complete_type(^^Crate<Shape>);
}
static_assert(probe(^^S<int>::In, ^^const S<int>::In *, ^^ns::R<char, 3>::Q::y, ^^S<int>::f));
