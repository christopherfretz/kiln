// [dcl.attr.grammar]/4 (GNU extension): an attribute kiln interprets is
// counted as clang counts it: from the argument table at parse time where
// kiln's handler would accept a wrong count or word it otherwise
// (`abi_tag`, `aligned`, `mode`, `format`, `section`), by its handler
// after the subject check otherwise (`tls_model` on a variable that is
// not thread-local, `destructor` on a variable: the subject error alone)
// (spec/parse.md §6.2 "Argument table", WP-1879). Verified against clang
// 19 and 22.
//@ requires: clang<22
//@ expect: compile-fail
//@ strict-diags
//@ diag: 25:16 error "'abi_tag' attribute takes at least 1 argument"
//@ diag: 26:3 error "'abi_tag' attribute takes at least 1 argument"
//@ diag: 27:16 error "'aligned' attribute takes no more than 1 argument"
//@ diag: 28:16 error "'mode' attribute takes one argument"
//@ diag: 29:3 error "'mode' attribute takes one argument"
//@ diag: 30:3 error "'format' attribute requires exactly 3 arguments"
//@ diag: 31:3 error "'section' attribute takes one argument"
//@ diag: 32:11 error "'lifetimebound' attribute takes no arguments"
//@ diag: 33:16 error "'alloc_size' attribute takes no more than 2 arguments"
//@ diag: 34:16 error "'tls_model' attribute takes one argument"
//@ diag: 35:16 error "'tls_model' attribute only applies to thread-local variables"
//@ diag: 36:16 warning "'destructor' attribute only applies to functions"

__attribute__((abi_tag)) void f1();
[[gnu::abi_tag]] void f2();
__attribute__((aligned(1, 2))) int v3;
__attribute__((mode(1, 2))) int v4;
[[gnu::mode]] int v5;
[[gnu::format(1)]] void f6();
[[gnu::section]] void f7();
void f8([[clang::lifetimebound(1)]] int &a);
__attribute__((alloc_size(1, 2, 3))) void *f9(int, int);
__attribute__((tls_model("local-exec", 1))) thread_local int v10;
__attribute__((tls_model)) int v11;
__attribute__((destructor(1, 2))) int v12;
