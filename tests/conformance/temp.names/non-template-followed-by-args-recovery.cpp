// [temp.names]/3: a single non-template found by name lookup followed by
// `<` type-id `>` is diagnosed ("'v' does not name a template but is
// followed by template arguments") and, as clang's
// checkPotentialAngleBracket does, makes the expression invalid
// (`ExprError`): the rest of a `= initializer` is skipped to the next
// declarator's `,` or the `;`, and the rest of an expression statement to
// its `;`, with no "expected ';'" for a following `{}`.
//@ expect: compile-fail
//@ std: c++17
//@ strict-diags
//@ diag: 29:13 error "'v' does not name a template but is followed by template arguments"
//@ diag: 28:5 note "non-template declaration found by name lookup"
//@ diag: 30:13 error "'v' does not name a template but is followed by template arguments"
//@ diag: 28:5 note "non-template declaration found by name lookup"
//@ diag: 30:25 error "'v' does not name a template but is followed by template arguments"
//@ diag: 28:5 note "non-template declaration found by name lookup"
//@ diag: 31:27 error "'p' does not name a template but is followed by template arguments"
//@ diag: 31:14 note "non-template declaration found by name lookup"
//@ diag: 31:40 error "'v' does not name a template but is followed by template arguments"
//@ diag: 28:5 note "non-template declaration found by name lookup"
//@ diag: 32:20 error "'v' does not name a template but is followed by template arguments"
//@ diag: 28:5 note "non-template declaration found by name lookup"
//@ diag: 32:32 error "'v' does not name a template but is followed by template arguments"
//@ diag: 28:5 note "non-template declaration found by name lookup"
//@ diag: 32:46 error "'v' does not name a template but is followed by template arguments"
//@ diag: 28:5 note "non-template declaration found by name lookup"
struct S {};
int v;
int a = 1 + v<S>{};
int b = 1 + v<S>{}, c = v<S>{}, d = 2;
void use(int p) { int q = p<long>(3) + v<S>{}; }
void f() { int e = v<S>{}; e = v<S>{}; (void)v<S>{}; }
