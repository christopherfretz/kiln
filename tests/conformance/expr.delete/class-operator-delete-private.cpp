// [expr.delete]/12, [class.access]/4: a class-scope deallocation function
// found for a delete-expression is access-checked from the context of
// the delete-expression (libc++ shared_ptr_Y.pass.cpp).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 9:16 error "'operator delete' is a private member of 'P'"
//@ diag: 8:16 note "implicitly declared private here"
class P { void operator delete(void *); };
void f(P *p) { delete p; }
