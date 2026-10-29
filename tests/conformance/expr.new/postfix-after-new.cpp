// [expr.new]/1: a new-expression is not a postfix-expression; a postfix
// operator after it needs parentheses.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 6:34 error "expression cannot be followed by a postfix '[' operator; add parentheses"
int f() { return new int[2]{1, 2}[0]; }
