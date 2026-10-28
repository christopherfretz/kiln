// [dcl.attr.deprecated]/4: using a deprecated entity is warned
//@ expect: compile-ok
//@ strict-diags
//@ diag: 10:9 warning "'a' is deprecated: use b"
//@ diag: 8:3 note "'a' has been explicitly marked deprecated here"
//@ diag: 11:15 warning "'old' is deprecated"
//@ diag: 9:3 note "'old' has been explicitly marked deprecated here"
[[deprecated("use b")]] int a;
[[deprecated]] void old();
int b = a;
void call() { old(); }
