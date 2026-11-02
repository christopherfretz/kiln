// [over.over]/3, [temp.arg.explicit]/4, [dcl.fct.def.delete]/2: a
// function template-id naming a deleted specialization is an overload
// set until it is used; the use is diagnosed at the start of the
// operand it forms (its `&` or parenthesis), and a call through it is a
// call to a deleted function.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 28:12 error "attempt to use a deleted function"
//@ diag: 25:23 note "'f<2>' has been explicitly marked deleted here"
//@ diag: 29:20 error "attempt to use a deleted function"
//@ diag: 26:25 note "'t<int>' has been explicitly marked deleted here"
//@ diag: 30:12 error "attempt to use a deleted function"
//@ diag: 25:23 note "'f<3>' has been explicitly marked deleted here"
//@ diag: 31:12 error "attempt to use a deleted function"
//@ diag: 24:37 note "'f<4>' has been explicitly marked deleted here"
//@ diag: 32:12 error "attempt to use a deleted function"
//@ diag: 25:23 note "'f<5>' has been explicitly marked deleted here"
//@ diag: 33:12 error "attempt to use a deleted function"
//@ diag: 25:23 note "'f<6>' has been explicitly marked deleted here"
//@ diag: 34:3 error "call to deleted function 'f'"
//@ diag: 25:23 note "candidate function [with I = 7] has been explicitly deleted"
//@ diag: 35:3 error "call to deleted function 'f'"
//@ diag: 24:37 note "candidate function [with I = 8] has been explicitly deleted"
namespace N { template <int I> void f() = delete; }
template <int I> void f() = delete;
template <class T> void t(T) = delete;
void d() {
  auto p = &f<2>;
  void (*v)(int) = &t<int>;
  auto a = &(f<3>);
  auto b = &N::f<4>;
  auto c = (&f<5>);
  auto e = f<6>;
  (f<7>)();
  (&N::f<8>)();
}
