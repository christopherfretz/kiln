// [dcl.fct.def.general]/1, [dcl.fct.def.delete]: `= delete` and `= delete("reason")` are function definitions
//@ expect: compile-fail
//@ diag: 5:19 error "'= delete' is a function definition and must occur in a standalone declaration"
//@ diag: 7:30 error "expected string literal in 'delete'"
void zz(), zy() = delete;
void ok() = delete("reason");
struct S { void l() = delete(1); };
