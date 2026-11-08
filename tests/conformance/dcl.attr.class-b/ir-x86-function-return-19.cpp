// [driver] `-mfunction-return=thunk-extern` on x86 gives every function
// `fn_ret_thunk_extern` and the "function_return_thunk_extern" module
// flag; `function_return("keep")` opts a function out
// (CodeGenModule::SetLLVMFunctionAttributesForDefinition;
// spec/landed/codegen/WP-813.md). WP-813.
//@ requires: clang<22
//@ expect: ir
//@ args: --target=x86_64-unknown-linux-gnu -mfunction-return=thunk-extern
void g();
__attribute__((function_return("keep"))) void keep() { g(); }
__attribute__((function_return("thunk-extern"))) void thunk() { g(); }
void plain() { g(); }
