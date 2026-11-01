// The type traits whose __has_builtin answer clang 19.1.0-19.1.2 got wrong
// (llvm/llvm-project#111477, fixed in 19.1.3): they answer 1, as kiln's
// feature table and clang 19.1.3+ do. Split from has-builtin-probe.cpp so
// an older oracle skips only these (docs/VALIDATION.md "Known oracle
// defects").
//@ expect: pp
//@ requires: clang>=19.1.3
__can_pass_in_regs __has_builtin(__can_pass_in_regs) __has_constexpr_builtin(__can_pass_in_regs)
__reference_binds_to_temporary __has_builtin(__reference_binds_to_temporary) __has_constexpr_builtin(__reference_binds_to_temporary)
__reference_constructs_from_temporary __has_builtin(__reference_constructs_from_temporary) __has_constexpr_builtin(__reference_constructs_from_temporary)
__reference_converts_from_temporary __has_builtin(__reference_converts_from_temporary) __has_constexpr_builtin(__reference_converts_from_temporary)
