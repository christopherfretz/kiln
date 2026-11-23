// [expr.call]/7, [dcl.attr.grammar]/5: the hidden size argument of a
// `pass_object_size` parameter is computed right after its argument,
// before the next argument is evaluated (clang `EmitCallArgs`,
// `MaybeEmitImplicitObjectSize`); kiln computes every size after all the
// arguments. WP-2476.
//@ expect: ir
//@ xfail(kiln): WP-2476 follow-up, the object sizes are emitted after all call arguments
int f(int *const v __attribute__((pass_object_size(1))), int w);
int use(int *a, int b) { return f(a, b); }
