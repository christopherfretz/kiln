// [dcl.type.class.deduct]/1: a deduced class type placeholder may only be
// the whole declared type; clang reports the outermost declarator chunk
// with err_deduced_class_template_compound_type.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 27:3 error "cannot form pointer to deduced class template specialization type"
//@ diag: 28:9 error "cannot form pointer to deduced class template specialization type"
//@ diag: 29:3 error "cannot use parentheses when declaring variable with deduced class template specialization type"
//@ diag: 30:3 error "cannot form reference to deduced class template specialization type"
//@ diag: 31:3 error "cannot form reference to deduced class template specialization type"
//@ diag: 32:5 error "cannot form array of deduced class template specialization type"
//@ diag: 33:3 error "cannot use parentheses when declaring variable with deduced class template specialization type"
//@ diag: 34:1 error "use of class template 'B' requires template arguments; argument deduction not allowed in function return type"
//@ diag: 23:26 note "template is declared here"
//@ diag: 35:3 error "cannot form pointer to deduced class template specialization type"
//@ diag: 36:16 error "cannot form pointer to deduced class template specialization type"
//@ diag: 37:3 error "cannot form pointer to deduced class template specialization type"
//@ diag: 38:3 error "cannot form pointer to deduced class template specialization type"
//@ diag: 39:21 error "use of class template 'B' requires template arguments; argument deduction not allowed in conversion function type"
//@ diag: 23:26 note "template is declared here"
//@ diag: 40:8 error "use of class template 'B' requires template arguments; argument deduction not allowed in function prototype"
//@ diag: 23:26 note "template is declared here"
template<class T> struct B { B(T); };
template<class T> struct Outer { template<class U> struct In { In(U); }; };
struct S {};
B<int> x(1);
B *p1 = &x;
B const *p2 = &x;
B (*p3) = &x;
B &r1 = x;
B &&r2 = B(1);
B a1[2] = {1, 2};
B (pa)(1);
B f1();
B S::*pm = nullptr;
Outer<int>::In *pi = nullptr;
B **pp = nullptr;
B *(*pf)() = nullptr;
struct E { operator B*(); };
void k(B *);
