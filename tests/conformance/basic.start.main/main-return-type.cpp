// [basic.start.main]/2: main returns int
//@ expect: compile-fail
//@ strict-diags
//@ diag: 5:1 error "'main' must return 'int'"
void main() {}
