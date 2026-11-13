// [cpp.pragma]: '#pragma weak alias = target' emits 'alias' as an LLVM
// 'weak alias' of the target (clang ActOnPragmaWeakAlias): a function's
// alias has a mangled name, a variable's does not; a target declared
// after the pragma takes its alias when it is declared. WP-1399.
//@ expect: ir
extern "C" int tgt_fn() { return 3; }
#pragma weak alias_fn = tgt_fn
#pragma weak alias_late = late_fn
extern "C" int late_fn() { return 4; }
extern "C" int tgt_var = 9;
#pragma weak alias_var = tgt_var
int use() { return alias_fn() + alias_late() + alias_var; }
