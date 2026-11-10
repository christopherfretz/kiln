//@ expect: compile-fail
//@ strict-diags
//@ diag: 6 error "kiln's own wording, which GCC does not share"
// Under GCC only the verdict and an error located in the test file count;
// the `diag:` text and `strict-diags` are kiln's and clang's.
int f() { return nope; }
