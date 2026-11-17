// [expr.ref]/7, [temp.dep.expr] (P2996R13): the rejected twin of
// member-splice-in-template-entities.cpp: a dependent member splice whose
// reflection designates no member of the object's class (nor of a base)
// is ill-formed in the instantiation that substitutes it, and only
// there. WP-1613.
//@ args: -freflection
//@ skip(clang): clang does not implement P2996 reflection
//@ oracle: gcc
//@ expect: compile-fail
//@ strict-diags
//@ diag: 16:14 error "splice in a member access position does not designate a member of 'D'"
//@ diag: 19:9 note "in instantiation of function template specialization 'g<^^O::z>' requested here"
struct D { int m; };
struct O { int z; };
template <decltype(^^int) R> int g(D d) {
  return d.[:R:];
}
int a = g<^^D::m>({1});
int b = g<^^O::z>({1});
