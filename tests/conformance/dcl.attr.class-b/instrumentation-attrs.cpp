// The instrumentation and XRay attributes' subjects and arguments as
// clang 19 checks them (xray_log_args's parameter index counts the
// implicit object parameter of a member), and __has_attribute reports
// each (spec/landed/codegen/WP-813.md). [dcl.attr.grammar]/5. WP-813.
//@ verify
__attribute__((no_instrument_function)) int v1; // expected-warning {{'no_instrument_function' attribute only applies to functions and Objective-C methods}}
__attribute__((no_instrument_function(1))) void f1(); // expected-error {{'no_instrument_function' attribute takes no arguments}}
__attribute__((no_profile_instrument_function)) int v2; // expected-warning {{'no_profile_instrument_function' attribute only applies to functions}}
__attribute__((xray_always_instrument)) int v3; // expected-warning {{'xray_always_instrument' attribute only applies to functions and Objective-C methods}}
__attribute__((xray_never_instrument(1))) void f3(); // expected-error {{'xray_never_instrument' attribute takes no arguments}}
__attribute__((xray_always_instrument, xray_never_instrument)) void f3b();
__attribute__((xray_log_args)) void f4(int); // expected-error {{'xray_log_args' attribute takes one argument}}
__attribute__((xray_log_args(0))) void f4b(int); // expected-error {{'xray_log_args' attribute parameter 1 is out of bounds}}
__attribute__((xray_log_args(2))) void f4c(int); // expected-error {{'xray_log_args' attribute parameter 1 is out of bounds}}
__attribute__((xray_log_args(1))) int v4; // expected-warning {{'xray_log_args' attribute only applies to functions and Objective-C methods}}
struct X { __attribute__((xray_log_args(1))) void m(int); __attribute__((xray_log_args(2))) void m2(int); };
[[clang::xray_always_instrument]] void ok1();
[[clang::xray_never_instrument]] void ok2();
[[gnu::no_instrument_function]] void ok3();
[[gnu::no_profile_instrument_function]] void ok4();
#if !__has_attribute(no_instrument_function) || !__has_attribute(no_profile_instrument_function) || !__has_attribute(xray_always_instrument) || !__has_attribute(xray_never_instrument) || !__has_attribute(xray_log_args) || !__has_cpp_attribute(clang::xray_log_args)
#error "not reported"
#endif

