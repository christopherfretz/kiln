// [expr.new]/19, [class.access]/4: a class-scope allocation function found
// for a new-expression is access-checked from the context of the
// new-expression (libc++ shared_ptr_Y.pass.cpp).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 9:17 error "'operator new' is a private member of 'P'"
//@ diag: 8:17 note "implicitly declared private here"
class P { void *operator new(__SIZE_TYPE__); };
P *g() { return new P; }
