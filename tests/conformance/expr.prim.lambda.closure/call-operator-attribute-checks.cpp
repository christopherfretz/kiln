// [expr.prim.lambda.closure]/7: the attribute-specifier-seq before the
// lambda-declarator, and the GNU attributes after its parameter clause,
// appertain to the call operator, so they are checked against it as
// against any member function (clang `ActOnStartOfLambdaDefinition` ->
// `ProcessDeclAttributes`): `nonnull` and `alloc_size` count the implicit
// object parameter, a deduced return type is not yet a pointer, and a
// subject that is not a function is reported. The linkage checks of
// declarators (`weak` with internal linkage) do not run.
//@ verify

auto l1 = [] (int *) __attribute__((nonnull(1))) {};        // expected-error {{'nonnull' attribute is invalid for the implicit this argument}}
auto l2 = [] (int *) __attribute__((nonnull(4))) {};        // expected-error {{'nonnull' attribute parameter 1 is out of bounds}}
auto l3 = [] (int *) __attribute__((nonnull(2))) {};
auto l4 = [] [[gnu::nonnull]] () {};                        // expected-warning {{attribute applied to function with no pointer arguments}}
auto l5 = [] (int) __attribute__((alloc_size(1))) { return (void *)0; }; // expected-warning {{attribute only applies to return values that are pointers}}
auto l6 = [] (int) __attribute__((alloc_size(2))) -> void * { return nullptr; };
auto l7 = [] (int) __attribute__((alloc_size(3))) -> void * { return nullptr; }; // expected-error {{'alloc_size' attribute parameter 1 is out of bounds}}
auto l8 = [] () __attribute__((visibility("bogus"))) {};    // expected-warning {{'visibility' attribute argument not supported: bogus}}
auto l9 = [] () __attribute__((tls_model("local-exec"))) {}; // expected-error {{'tls_model' attribute only applies to thread-local variables}}
auto la = [] () __attribute__((cleanup(nofn))) {};          // expected-error {{use of undeclared identifier 'nofn'}}
auto lb = [] () __attribute__((weak)) {};
auto lc = [] [[deprecated("old")]] () {};                   // expected-note {{'operator()' has been explicitly marked deprecated here}}
auto ld = [] [[nodiscard]] () { return 1; };
auto le = [] () __attribute__((section("lsec"))) {};

void use() {
  lc();  // expected-warning {{'operator()' is deprecated: old}}
  ld();  // expected-warning {{ignoring return value of function declared with 'nodiscard' attribute}}
}
