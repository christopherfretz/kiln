// The x86-only attributes (nocf_check with -fcf-protection,
// function_return, force_align_arg_pointer): subjects and arguments as
// clang 19 checks them, and __has_attribute reports them on x86
// (spec/landed/codegen/WP-813.md). [dcl.attr.grammar]/5. WP-813.
//@ verify
//@ args: --target=x86_64-unknown-linux-gnu -fcf-protection=full
__attribute__((nocf_check)) int v1; // expected-warning {{'nocf_check' attribute only applies to functions and function pointers}}
__attribute__((nocf_check(1))) void f1(); // expected-error {{'nocf_check' attribute takes no arguments}}
__attribute__((function_return("bogus"))) void f2(); // expected-warning {{'function_return' attribute argument not supported: bogus}}
__attribute__((function_return)) void f2b(); // expected-error {{'function_return' attribute takes one argument}}
__attribute__((function_return("keep"))) int v2; // expected-warning {{'function_return' attribute only applies to functions}}
__attribute__((force_align_arg_pointer)) int v3; // expected-warning {{'force_align_arg_pointer' attribute only applies to functions}}
__attribute__((force_align_arg_pointer(1))) void f3(); // expected-error {{'force_align_arg_pointer' attribute takes no arguments}}
void (*__attribute__((nocf_check)) fp)();
__attribute__((nocf_check, function_return("thunk-extern"), force_align_arg_pointer)) void ok();
#if !__has_attribute(nocf_check) || !__has_attribute(function_return) || !__has_attribute(force_align_arg_pointer)
#error "not reported"
#endif

