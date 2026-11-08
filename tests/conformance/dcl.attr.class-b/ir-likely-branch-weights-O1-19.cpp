// [dcl.attr.likelihood] At -O1 and above clang 19 gives the branch of a
// [[likely]] / [[unlikely]] statement (and __builtin_expect) the
// `!prof !{!"branch_weights", !"expected", i32 1, i32 2000}` weights
// (CodeGenFunction::EmitBranchOnBoolExpr, getLikelihoodWeights); kiln
// emits none. At -O0 neither does (tests/codegen/likelihood_attrs.cpp).
// WP-813.
//@ requires: clang<22
//@ expect: ir
//@ args: -O1
//@ xfail(kiln): WP-813 follow-up, no branch weights for [[likely]]/[[unlikely]]/__builtin_expect when optimizing
void g();
void h();
void f(int x) {
  if (x) [[likely]] g(); else h();
}
