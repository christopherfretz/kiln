// [module.import]/1, [cpp.import]: importing a module whose interface
// cannot be found (no -fmodule-file, -fprebuilt-module-path or libc++
// module source for it) is a fatal error at the module name, as clang.
//@ expect: compile-fail
//@ std: c++20
//@ strict-diags
//@ diag: 8:8 fatal "module 'kiln.not_a_module' not found"
import kiln.not_a_module;
int ok;
