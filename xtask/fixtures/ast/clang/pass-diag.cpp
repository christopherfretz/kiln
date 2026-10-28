//@ expect: ast
//@ diag: 4 warning "unused"
//@ args: -Wunused-variable
void f() { int unused_var; }
