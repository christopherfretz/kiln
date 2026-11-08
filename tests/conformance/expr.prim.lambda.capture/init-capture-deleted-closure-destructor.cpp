// [class.dtor]/15, [expr.prim.lambda.closure]: a temporary whose destructor
// is deleted is an error expression (clang's MaybeBindToTemporary gives
// ExprError), so the variable it initializes is not checked again. An
// init-capture of a class with a protected destructor deletes the closure's
// destructor: the lambda temporary is diagnosed once, not again at the
// variable (WP-834).
//@ expect: compile-fail
//@ args: -ferror-limit=0
//@ strict-diags
//@ diag: 29:12 error "attempt to use a deleted function"
//@ diag: 29:13 note ".cpp:29:12)' is implicitly deleted because field '' has an inaccessible destructor"
//@ diag: 29:17 error "temporary of type 'B' has protected destructor"
//@ diag: 25:23 note "declared protected here"
//@ diag: 32:12 error "attempt to use a deleted function"
//@ diag: 32:13 note ".cpp:32:12)' is implicitly deleted because field '' has an inaccessible destructor"
//@ diag: 32:17 error "temporary of type 'B' has protected destructor"
//@ diag: 25:23 note "protected destructor can only be used to destroy a base class subobject"
//@ diag: 34:55 error "temporary of type 'B' has protected destructor"
//@ diag: 35:15 note "in instantiation of function template specialization 'in_template<B>' requested here"
//@ diag: 25:23 note "declared protected here"
//@ diag: 34:50 error "attempt to use a deleted function"
//@ diag: 34:51 note ".cpp:34:50)' is implicitly deleted because field '' has an inaccessible destructor"
//@ diag: 36:37 error "attempt to use a deleted function"
//@ diag: 27:12 note "'~X' has been explicitly marked deleted here"
struct B { protected: ~B(); };
struct D : B { void f(); };
struct X { ~X() = delete; };
void at_block_scope() {
  auto l = [x = B()] { return 0; };
}
void D::f() {
  auto l = [x = B()] { return 0; };
}
template <class T> void in_template() { auto l = [x = T()] {}; }
template void in_template<B>();
void deleted_temporary() { int i = (X(), 0); }
