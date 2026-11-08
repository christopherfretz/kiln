// Off x86 the x86-only attributes are unknown and ignored, and
// nocf_check without -fcf-protection is ignored with a hint, as clang 19
// does (spec/landed/codegen/WP-813.md). [dcl.attr.grammar]/5. WP-813.
//@ verify
//@ args: --target=aarch64-unknown-linux-gnu
__attribute__((nocf_check)) void f1(); // expected-warning {{'nocf_check' attribute ignored; use -fcf-protection to enable the attribute}}
__attribute__((function_return("keep"))) void f2(); // expected-warning {{unknown attribute 'function_return' ignored}}
__attribute__((force_align_arg_pointer)) void f3(); // expected-warning {{unknown attribute 'force_align_arg_pointer' ignored}}
#if __has_attribute(nocf_check) || __has_attribute(function_return) || __has_attribute(force_align_arg_pointer)
#error "reported off x86"
#endif
