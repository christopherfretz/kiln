// [driver] Without `-mfunction-return=`, `function_return("thunk-extern")`
// still gives its function `fn_ret_thunk_extern`, and no module flag is
// emitted; force_align_arg_pointer gives "stackrealign"
// (spec/landed/codegen/WP-813.md). WP-813.
//@ requires: clang<22
//@ expect: ir
//@ args: --target=x86_64-unknown-linux-gnu
void g();
__attribute__((function_return("thunk-extern"))) void thunk() { g(); }
__attribute__((function_return("keep"))) void keep() { g(); }
__attribute__((force_align_arg_pointer)) void aligned() { g(); }
