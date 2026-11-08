// The pointer-fact attributes' subjects and arguments as clang 19
// checks them: assume_aligned, align_value (a typedef shows its
// underlying type), callback's callee and argument indices or names,
// noescape and maybe_undef; __has_attribute reports each
// (spec/landed/codegen/WP-813.md). [dcl.attr.grammar]/5. WP-813.
//@ verify
//@ requires: clang<22
void *__attribute__((assume_aligned(3))) a1(); // expected-error {{requested alignment is not a power of 2}}
void *__attribute__((assume_aligned)) a2(); // expected-error {{'assume_aligned' attribute takes at least 1 argument}}
int __attribute__((assume_aligned(8))) a3(); // expected-warning {{'assume_aligned' attribute only applies to return values that are pointers or references}}
void *__attribute__((assume_aligned(8, 2, 1))) a4(); // expected-error {{'assume_aligned' attribute takes no more than 2 arguments}}
typedef int *__attribute__((align_value(3))) av1; // expected-error {{requested alignment is not a power of 2}}
typedef int __attribute__((align_value(8))) av2; // expected-warning {{'align_value' attribute only applies to a pointer or reference ('int' is invalid)}}
void av3(int *__attribute__((align_value(16))) p);
void cb1(void (*)(void *), void *) __attribute__((callback(3, 2))); // expected-error {{'callback' attribute parameter 1 is out of bounds}}
void cb2(int, void *) __attribute__((callback(1, 2))); // expected-error {{'callback' attribute callee does not have function type}}
void cb3(void (*)(void *), void *) __attribute__((callback(1))); // expected-error {{'callback' attribute takes no arguments}}
void cb4(void (*)(void *), void *) __attribute__((callback(f, 2))); // expected-error {{'callback' attribute argument 'f' is not a known function parameter}}
void ne1(__attribute__((noescape)) int p); // expected-warning {{'noescape' attribute only applies to pointer arguments}}
__attribute__((noescape)) int ne2; // expected-warning {{'noescape' attribute only applies to parameters}}
void ne3(__attribute__((noescape)) int &p);
void mu1(__attribute__((maybe_undef)) int p);
__attribute__((maybe_undef)) int mu2; // expected-warning {{'maybe_undef' attribute only applies to parameters}}
void ok2(void (*cbk)(void *), void *p) __attribute__((callback(cbk, p)));
#if !__has_attribute(assume_aligned) || !__has_attribute(align_value) || !__has_attribute(callback) || !__has_attribute(noescape) || !__has_attribute(maybe_undef) || !__has_cpp_attribute(clang::noescape) || !__has_cpp_attribute(clang::callback)
#error "not reported"
#endif

