// WP-813: the class B attributes on declarations: clang 19 puts only
// ConstructAttributeList's ones ("zero-call-used-regs",
// `speculative_load_hardening`, and on x86 `nocf_check`) on the
// `declare` and its calls; the definition-only ones (patchable entry,
// XRay, noprofile, nouwtable, flatten, ...) leave the declaration alone.
// spec/landed/codegen/WP-813.md.
__attribute__((no_stack_protector)) void d1();
__attribute__((patchable_function_entry(3))) void d2();
__attribute__((zero_call_used_regs("all"))) void d4();
__attribute__((speculative_load_hardening)) void d5();
__attribute__((no_profile_instrument_function)) void d9();
__attribute__((xray_always_instrument)) void d10();
__attribute__((xray_never_instrument)) void d11();
__attribute__((disable_sanitizer_instrumentation)) void d13();
__attribute__((nocf_check)) void d14();
__attribute__((function_return("thunk-extern"))) void d15();
__attribute__((cfi_canonical_jump_table)) void d16();
__attribute__((force_align_arg_pointer)) void d17();
__attribute__((nouwtable)) void d18();
__attribute__((flatten)) void d19();
__attribute__((no_instrument_function)) void d20();
void use() { d1(); d2(); d4(); d5(); d9(); d10(); d11(); d13(); d14(); d15(); d16(); d17(); d18(); d19(); d20();}
