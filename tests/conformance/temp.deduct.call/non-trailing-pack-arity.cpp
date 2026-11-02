// [temp.deduct.call]/1: a function parameter pack that is not trailing is
// a non-deduced context, deduced empty unless given explicitly; a call
// with more arguments is not rejected before deduction (clang
// `isTemplateVariadic`) but by the viability of the deduced
// specialization.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 12:3 error "no matching function for call to 'f'"
//@ diag: 10:36 note "candidate function [with U = int, T = <>] not viable: requires 1 argument, but 3 were provided"
template<class U, class... T> void f(T..., U);
void g() {
  f(1, 2, 3);
  f<int, char, long>('a', 2L, 3);
}
