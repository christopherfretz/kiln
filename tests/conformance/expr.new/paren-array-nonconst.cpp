// [expr.new]/1: a parenthesized type-id cannot have a runtime bound
// (clang accepts it as an extension, with a warning).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 7:26 warning "when type is in parentheses, array cannot have dynamic size"
//@ diag: 9:19 error "cannot initialize return object of type 'int *' with an lvalue of type 'int'"
void f(int n) { new (int[n]); }
int g;
int *h() { return g; }
