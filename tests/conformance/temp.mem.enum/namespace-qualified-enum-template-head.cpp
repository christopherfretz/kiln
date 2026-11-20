// [temp.pre]/2, [temp.mem.enum]: a template head before a qualified
// enumeration matches the class templates of its qualifier; one left over
// (the qualifier names a namespace) would make the enumeration a template:
// "enumeration cannot be a template" at `enum` (clang's `err_enum_template`).
// WP-1740.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 15:20 error "enumeration cannot be a template"
//@ diag: 16:31 error "forward declaration of enum cannot have a nested name specifier"
//@ diag: 16:20 error "enumeration cannot be a template"
namespace N { enum class Q : int; enum class R : int; }
template <class T> struct A { enum class F : int; };
template <class T> enum class A<T>::F : int { b };

template <class T> enum class N::Q : int { z };
template <class T> enum class N::R : int;

int f() { return int(A<int>::F::b); }
