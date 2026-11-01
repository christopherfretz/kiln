// [dcl.type.class.deduct]/2, [expr.new]/2: a placeholder for a deduced
// class type cannot be an array element type in a new-expression; clang
// reports the innermost array (also in a template definition).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 12:18 error "cannot form array of deduced class template specialization type"
//@ diag: 13:16 error "cannot form array of deduced class template specialization type"
//@ diag: 15:18 error "cannot form array of deduced class template specialization type"
//@ diag: 16:20 error "cannot form array of deduced class template specialization type"
//@ diag: 17:46 error "cannot form array of deduced class template specialization type"
template<class T> struct A { A(T); };
auto *pa = new A[2]{1, 2};
auto *pb = new A[]{1, 2};
int n = 2;
auto *pc = new A[n]{1, 2};
auto *pd = new A[2][3]{1, 2};
template<class T> void f() { auto *p = new A[sizeof(T)]{1}; }
