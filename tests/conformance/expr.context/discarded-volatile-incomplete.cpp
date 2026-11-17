// [expr.context]/2 with [conv.lval]/1: from C++11 a discarded-value
// expression that is a glvalue of volatile-qualified type and an
// id-expression, subscript, class member access, indirection,
// pointer-to-member operation, or a comma or conditional of those
// undergoes the lvalue-to-rvalue conversion, which needs a complete type:
// a class template specialization is instantiated (and its errors are
// reported), and a class that stays incomplete is an error. Found running
// GCC's C++ testsuite; clang 19 and 22 neither complete nor diagnose; GCC
// 16 instantiates but only warns for a class that stays incomplete: the
// text decides.
//@ xfail(clang): clang 19 and 22 do not require a complete type for a discarded volatile glvalue
//@ expect: compile-fail
//@ std: c++20
//@ strict-diags
//@ diag: 30:38 error "field has incomplete type 'I'"
//@ diag: 38:3 note "in instantiation of template class 'H<I, 0>' requested here"
//@ diag: 29:8 note "forward declaration of 'I'"
//@ diag: 30:38 error "field has incomplete type 'I'"
//@ diag: 39:3 note "in instantiation of template class 'H<I, 1>' requested here"
//@ diag: 29:8 note "forward declaration of 'I'"
//@ diag: 40:3 error "incomplete type 'volatile I' where a complete type is required"
//@ diag: 41:5 error "incomplete type 'volatile I' where a complete type is required"
//@ diag: 42:4 error "incomplete type 'volatile I' where a complete type is required"
//@ diag: 43:5 error "incomplete type 'volatile I' where a complete type is required"
//@ diag: 44:3 error "incomplete type 'volatile I' where a complete type is required"
//@ diag: 45:9 error "incomplete type 'volatile I' where a complete type is required"
//@ diag: 42:4 warning "expression result unused [-Wunused-value]"
//@ diag: 44:4 warning "left operand of comma operator has no effect [-Wunused-value]"
struct I;
template <class T, int> struct H { T t; };
extern volatile H<I, 0> a;
extern volatile H<I, 1> &b;
extern volatile I *p;
struct S { volatile I &r; };
extern S s;
extern volatile I S::*pm;
void t(bool c) {
  a;
  b;
  *p;
  s.r;
  s.*pm;
  c ? *p : s.r;
  (0, *p);
  (void)*p;
}
