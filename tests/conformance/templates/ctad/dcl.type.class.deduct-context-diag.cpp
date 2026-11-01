// [dcl.type.class.deduct]/2: a deduced class type placeholder is not
// allowed outside an initialized declaration, a new-type-id or an
// explicit type conversion; clang's err_auto_not_allowed context selects.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 39:9 error "use of class template 'B' requires template arguments; argument deduction not allowed in function prototype"
//@ diag: 38:26 note "template is declared here"
//@ diag: 40:14 error "use of class template 'B' requires template arguments; argument deduction not allowed in function prototype"
//@ diag: 38:26 note "template is declared here"
//@ diag: 42:3 error "use of class template 'B' requires template arguments; argument deduction not allowed in non-static struct member"
//@ diag: 38:26 note "template is declared here"
//@ diag: 43:10 error "use of class template 'B' requires template arguments; argument deduction not allowed in function prototype"
//@ diag: 38:26 note "template is declared here"
//@ diag: 44:3 error "use of class template 'B' requires template arguments; argument deduction not allowed in function return type"
//@ diag: 38:26 note "template is declared here"
//@ diag: 45:10 error "use of class template 'B' requires template arguments; argument deduction not allowed in function return type"
//@ diag: 38:26 note "template is declared here"
//@ diag: 46:13 error "use of class template 'B' requires template arguments; argument deduction not allowed in non-static union member"
//@ diag: 38:26 note "template is declared here"
//@ diag: 47:13 error "use of class template 'B' requires template arguments; argument deduction not allowed in non-static class member"
//@ diag: 38:26 note "template is declared here"
//@ diag: 48:10 error "use of class template 'B' requires template arguments; argument deduction not allowed in friend declaration"
//@ diag: 38:26 note "template is declared here"
//@ diag: 49:12 error "use of class template 'B' requires template arguments; argument deduction not allowed in conversion function type"
//@ diag: 38:26 note "template is declared here"
//@ diag: 51:9 error "use of class template 'B' requires template arguments; argument deduction not allowed in typedef"
//@ diag: 38:26 note "template is declared here"
//@ diag: 52:26 error "use of class template 'B' requires template arguments; argument deduction not allowed in exception declaration"
//@ diag: 38:26 note "template is declared here"
//@ diag: 53:13 error "use of class template 'B' requires template arguments; argument deduction not allowed in lambda parameter"
//@ diag: 38:26 note "template is declared here"
//@ diag: 54:20 error "use of class template 'B' requires template arguments; argument deduction not allowed in requires expression parameter"
//@ diag: 38:26 note "template is declared here"
//@ diag: 55:37 error "use of class template 'B' requires template arguments; argument deduction not allowed in function prototype"
//@ diag: 38:26 note "template is declared here"
//@ diag: 56:27 error "use of class template 'B' requires template arguments; argument deduction not allowed here"
//@ diag: 38:26 note "template is declared here"
template<class T> struct B { B(T); };
void g1(B);
void g2(int, B b);
struct S {
  B m;
  void f(B);
  B g();
  static B h();
  union U { B u; };
  class K { B k; };
  friend B;
  operator B();
};
typedef B TD;
void h() { try {} catch (B) {} }
auto l = [](B) {};
bool rq = requires(B b) { b; };
template<class T> struct D { void f(B); };
auto f3() { return sizeof(B); }
