// C17 6.7p3, 6.9p5 (constraints on redeclarations): a declaration that
// violates one is invalid, as clang's `MergeFunctionDecl` and
// `MergeVarDecl` leave it. A `static` definition after a non-static one
// gets the one error (no second "redefinition"), and a block-scope
// redefinition with another type leaves the earlier variable the one
// later uses name.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 15:12 error "static declaration of 'b' follows non-static declaration"
//@ diag: 14:5 note "previous"
//@ diag: 19:10 error "redefinition of 'z' with a different type: 'double' vs 'int'"
//@ diag: 18:7 note "previous definition is here"
//@ diag: 20:4 error "member reference base type 'int' is not a structure or union"
int b(void) { return 1; }
static int b(void) { return 2; }
int b(void);
void g(void) {
  int z;
  double z;
  z.x = 1;
}
