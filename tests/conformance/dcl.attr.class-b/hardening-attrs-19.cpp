// The hardening attributes' subjects and arguments as clang 19 checks
// them (Attr.td subjects, handlePatchableFunctionEntryAttr,
// handleZeroCallUsedRegsAttr, the speculative_load_hardening
// MutualExclusions on one declaration and across redeclarations), and
// __has_attribute reports each (spec/landed/codegen/WP-813.md).
// [dcl.attr.grammar]/5 (implementation-defined attributes). WP-813.
//@ verify
//@ requires: clang<22
__attribute__((no_stack_protector)) int v1; // expected-warning {{'no_stack_protector' attribute only applies to functions}}
__attribute__((no_stack_protector(1))) void f1(); // expected-error {{'no_stack_protector' attribute takes no arguments}}
__attribute__((patchable_function_entry)) void f2(); // expected-error {{'patchable_function_entry' attribute takes at least 1 argument}}
__attribute__((patchable_function_entry(-1))) void f2b(); // expected-error {{'patchable_function_entry' attribute requires a non-negative integral compile time constant expression}}
__attribute__((patchable_function_entry(1, 2, 3))) void f2c(); // expected-error {{'patchable_function_entry' attribute takes no more than 2 arguments}}
__attribute__((patchable_function_entry(1))) int v2; // expected-warning {{'patchable_function_entry' attribute only applies to functions and Objective-C methods}}
__attribute__((patchable_function_entry(1, 2))) void f2d(); // expected-error {{'patchable_function_entry' attribute requires integer constant between 0 and 1 inclusive}}
__attribute__((zero_call_used_regs("bogus"))) void f3(); // expected-warning {{'zero_call_used_regs' attribute argument not supported: bogus}}
__attribute__((zero_call_used_regs)) void f3b(); // expected-error {{'zero_call_used_regs' attribute takes one argument}}
__attribute__((zero_call_used_regs(1))) void f3c(); // expected-error {{expected string literal as argument of 'zero_call_used_regs' attribute}}
__attribute__((zero_call_used_regs("all"))) int v3; // expected-error {{'zero_call_used_regs' attribute only applies to functions}}
__attribute__((speculative_load_hardening)) int v4; // expected-error {{'speculative_load_hardening' attribute only applies to functions and Objective-C methods}}
__attribute__((speculative_load_hardening, no_speculative_load_hardening)) void f4(); // expected-error {{'no_speculative_load_hardening' and 'speculative_load_hardening' attributes are not compatible}} expected-note {{conflicting attribute is here}}
__attribute__((no_speculative_load_hardening)) void f5(); // expected-note {{conflicting attribute is here}}
__attribute__((speculative_load_hardening)) void f5(); // expected-error {{'speculative_load_hardening' and 'no_speculative_load_hardening' attributes are not compatible}}
__attribute__((no_split_stack)) int v6; // expected-error {{'no_split_stack' attribute only applies to functions}}
__attribute__((no_split_stack(1))) void f6(); // expected-error {{'no_split_stack' attribute takes no arguments}}
__attribute__((cfi_canonical_jump_table)) int v7; // expected-error {{'cfi_canonical_jump_table' attribute only applies to functions}}
__attribute__((disable_sanitizer_instrumentation)) int v8;
struct __attribute__((no_stack_protector)) S1 {}; // expected-warning {{'no_stack_protector' attribute only applies to functions}}
void ok1() __attribute__((no_stack_protector, patchable_function_entry(2, 1), zero_call_used_regs("used-gpr-arg")));
#if !__has_attribute(no_stack_protector) || !__has_attribute(patchable_function_entry) || !__has_attribute(zero_call_used_regs) || !__has_attribute(speculative_load_hardening) || !__has_attribute(no_speculative_load_hardening) || !__has_attribute(no_split_stack) || !__has_attribute(cfi_canonical_jump_table) || !__has_attribute(disable_sanitizer_instrumentation) || !__has_cpp_attribute(clang::no_stack_protector) || !__has_cpp_attribute(gnu::no_stack_protector) || !__has_cpp_attribute(gnu::zero_call_used_regs)
#error "not reported"
#endif

