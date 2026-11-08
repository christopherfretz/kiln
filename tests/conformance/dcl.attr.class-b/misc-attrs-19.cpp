// code_align, loader_uninitialized, lto_visibility_public, omp::assume
// and standalone_debug: subjects and arguments as clang 19 checks them,
// and __has_attribute (spec/landed/codegen/WP-813.md).
// [dcl.attr.grammar]/5. WP-813.
//@ verify
//@ requires: clang<22
void s(int *a) {
  [[clang::code_align(3)]] for (int i = 0; i < 4; ++i) a[i] = 0; // expected-error {{'code_align' attribute requires an integer argument which is a constant power of two between 1 and 4096 inclusive; provided argument was 3}}
  [[clang::code_align(8192)]] for (int i = 0; i < 4; ++i) a[i] = 0; // expected-error {{'code_align' attribute requires an integer argument which is a constant power of two between 1 and 4096 inclusive; provided argument was 8192}}
  [[clang::code_align]] for (int i = 0; i < 4; ++i) a[i] = 0; // expected-error {{'code_align' attribute takes one argument}}
  [[clang::code_align(8)]] a[0] = 1; // expected-error {{'code_align' attribute only applies to 'for', 'while', and 'do' statements}}
  [[clang::code_align(8), clang::code_align(16)]] while (*a) ++a; // expected-error {{conflicting loop attribute 'code_align'}} expected-note {{previous attribute is here}}
}
int lu1 __attribute__((loader_uninitialized)) = 1; // expected-error {{variable with 'loader_uninitialized' attribute cannot have an initializer}}
void lu2() { int x __attribute__((loader_uninitialized)); (void)x; } // expected-warning {{'loader_uninitialized' attribute only applies to global variables}}
__attribute__((loader_uninitialized)) void lu3(); // expected-warning {{'loader_uninitialized' attribute only applies to global variables}}
extern int lu4 __attribute__((loader_uninitialized)); // expected-error {{variable 'lu4' cannot be declared both 'extern' and with the 'loader_uninitialized' attribute}}
struct [[clang::lto_visibility_public]] LV {};
[[clang::lto_visibility_public]] int lv2; // expected-warning {{'lto_visibility_public' attribute only applies to structs, unions, and classes}}
[[omp::assume("omp_no_openmp")]] int oa1; // expected-warning {{'assume' attribute only applies to functions and Objective-C methods}}
[[omp::assume("bogus_thing")]] void oa2(); // expected-warning {{unknown assumption string 'bogus_thing'; attribute is potentially ignored}}
[[omp::assume(1)]] void oa3(); // expected-error {{expected string literal as argument of 'assume' attribute}}
struct [[clang::standalone_debug]] SD {};
[[clang::standalone_debug]] int sd2; // expected-warning {{'standalone_debug' attribute only applies to classes}}
#if !__has_attribute(code_align) || !__has_attribute(loader_uninitialized) || !__has_cpp_attribute(clang::lto_visibility_public) || !__has_attribute(standalone_debug) || !__has_cpp_attribute(omp::assume)
#error "not reported"
#endif

