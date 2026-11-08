// WP-813: the class B hardening and instrumentation attributes on
// definitions, as clang 19's CodeGenModule::SetLLVMFunctionAttributes-
// ForDefinition / CodeGenFunction::StartFunction emit them without any
// flag: "patchable-function-entry" / "patchable-function-prefix" (none
// for a count of 0), "zero-call-used-regs" (also "skip"),
// `speculative_load_hardening`, `noprofile`, "function-instrument"=
// "xray-never" (xray_always_instrument and xray_log_args need
// -fxray-instrument), `disable_sanitizer_instrumentation` and
// "cfi-canonical-jump-table". no_stack_protector, no_split_stack,
// no_instrument_function and no_speculative_load_hardening only cancel
// a flag (class_b_flag_*.cpp); nocf_check, function_return and
// force_align_arg_pointer act on x86 only. spec/landed/codegen/WP-813.md.
__attribute__((no_stack_protector)) void f1() { char b[100]; asm volatile(""::"r"(b)); }
[[clang::no_stack_protector]] void f1b() {}
[[gnu::no_stack_protector]] void f1c() {}
__attribute__((patchable_function_entry(3))) void f2() {}
__attribute__((patchable_function_entry(3,1))) void f3() {}
__attribute__((patchable_function_entry(0))) void f3b() {}
__attribute__((zero_call_used_regs("all"))) void f4() {}
__attribute__((zero_call_used_regs("skip"))) void f4b() {}
__attribute__((speculative_load_hardening)) void f5() {}
__attribute__((no_speculative_load_hardening)) void f6() {}
__attribute__((no_split_stack)) void f7() {}
__attribute__((no_instrument_function)) void f8() {}
__attribute__((no_profile_instrument_function)) void f9() {}
__attribute__((xray_always_instrument)) void f10() {}
__attribute__((xray_never_instrument)) void f11() {}
__attribute__((xray_always_instrument, xray_log_args(1))) void f12(int) {}
__attribute__((disable_sanitizer_instrumentation)) void f13() {}
__attribute__((nocf_check)) void f14() {}
__attribute__((function_return("thunk-extern"))) void f15() {}
__attribute__((function_return("keep"))) void f15b() {}
__attribute__((cfi_canonical_jump_table)) void f16() {}
__attribute__((force_align_arg_pointer)) void f17() {}
