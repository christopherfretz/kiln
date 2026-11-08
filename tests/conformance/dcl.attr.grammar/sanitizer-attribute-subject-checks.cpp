//@ expect: compile-fail
//@ std: c++20
//@ strict-diags
//@ diag: 13:28 error "expected string literal as argument of 'no_sanitize' attribute"
//@ diag: 16:16 error "'no_sanitize_memory' attribute takes no arguments"
//@ diag: 17:28 warning "unknown sanitizer 'bogus' ignored"
// [dcl.attr.grammar]/6 (implementation-defined attributes): clang's
// subject and argument checks for the sanitizer attributes. `no_sanitize`
// takes string literals (an unknown sanitizer is -Wunknown-sanitizers),
// `no_sanitize_memory` takes none;
// `no_sanitize("address")` and `no_sanitize_address` also apply to globals.
// WP-673 item 2, pinned by WP-873 (the `lifetimebound` checks are WP-674).
__attribute__((no_sanitize(1))) int f1();
__attribute__((no_sanitize("address"))) int v1;
__attribute__((no_sanitize_address)) int v2;
__attribute__((no_sanitize_memory(1))) int f2();
__attribute__((no_sanitize("bogus"))) int f3();
int &f4(int &x [[clang::lifetimebound]]);
