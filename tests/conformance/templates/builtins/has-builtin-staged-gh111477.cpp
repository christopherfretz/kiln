// The staged (sema-mode) __has_builtin answer of the traits clang
// 19.1.0-19.1.2 misreport (llvm/llvm-project#111477, fixed in 19.1.3):
// split from has-builtin-staged.cpp so an older clang self-check skips only
// these (docs/VALIDATION.md "Known oracle defects").
//@ strict-diags
//@ requires: clang>=19.1.3
#if !__has_builtin(__can_pass_in_regs)
#error __can_pass_in_regs
#endif
#if !__has_builtin(__reference_binds_to_temporary)
#error __reference_binds_to_temporary
#endif
#if !__has_builtin(__reference_constructs_from_temporary)
#error __reference_constructs_from_temporary
#endif
#if !__has_builtin(__reference_converts_from_temporary)
#error __reference_converts_from_temporary
#endif
