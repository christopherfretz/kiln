// A statement expression is only allowed inside a function body (clang's
// err_stmtexpr_file_scope, at the '('); the rest up to the ')' is skipped.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 8:9 error "statement expression not allowed at file scope"
//@ diag: 9:13 error "statement expression not allowed at file scope"
//@ diag: 10:14 error "default argument may not use a GNU statement expression"
int x = ({ 1; });
int y = 2 + ({ int z = 3; z; });
void h(int = ({ 4; }));
int ok() { return ({ 5; }); }
