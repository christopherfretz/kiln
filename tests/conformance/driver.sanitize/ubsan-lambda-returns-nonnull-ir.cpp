// `__attribute__((returns_nonnull))` after a lambda's parameter list
// appertains to the closure's function type, not to its call operator
// ([expr.prim.lambda.closure]/7): clang emits no returns-nonnull check
// in `operator()` and no `nonnull` return attribute.
// spec/codegen.md "Sanitizers", docs/design/sanitizers.md §2.6, WP-871.
//@ expect: ir
//@ std: c++20
//@ args: -fsanitize=returns-nonnull-attribute -Xclang -disable-llvm-passes
//@ xfail(kiln): WP-871 follow-up, sema puts a lambda's trailing returns_nonnull on operator()
int g;
auto l = [](int *p) __attribute__((returns_nonnull)) { return p; };
int *u() { return l(&g); }
