// [dcl.attr.grammar]/4 (GNU extension): the attribute-token determines the
// requirements on its attribute-argument-clause. kiln checks the argument
// count of every attribute clang knows against a table generated from
// clang 19 and 22 (spec/parse.md §6.2 "Argument table", WP-1879), with
// clang's wording, and drops the attribute. Verified against clang 19 and 22.
//@ requires: clang<22
//@ expect: compile-fail
//@ strict-diags
//@ diag: 21:16 error "'cold' attribute takes no arguments"
//@ diag: 22:16 error "'__cold__' attribute takes no arguments"
//@ diag: 23:3 error "'cold' attribute takes no arguments"
//@ diag: 24:3 error "'__hot__' attribute takes no arguments"
//@ diag: 25:16 error "'used' attribute takes no arguments"
//@ diag: 27:3 error "'minsize' attribute takes no arguments"
//@ diag: 28:16 error "'nothrow' attribute takes no arguments"
//@ diag: 29:16 error "'alloc_size' attribute takes at least 1 argument"
//@ diag: 30:16 error "'alloc_size' attribute takes no more than 2 arguments"
//@ diag: 31:16 error "'format' attribute requires exactly 3 arguments"
//@ diag: 32:16 error "'reqd_work_group_size' attribute requires exactly 3 arguments"

__attribute__((cold(1))) void f1();
__attribute__((__cold__(1, 2))) void f2();
[[gnu::cold(1)]] void f3();
[[gnu::__hot__("s")]] void f4();
__attribute__((used(1))) static void f5() {}
__attribute__((noinline())) void f6();
[[clang::minsize(0)]] void f7();
__attribute__((nothrow(1), cold)) void f8();
__attribute__((alloc_size)) void *f9(int);
__attribute__((alloc_size(1, 2, 1))) void *f10(int, int);
__attribute__((format(printf, 1))) void f11(const char *, ...);
__attribute__((reqd_work_group_size(1, 2))) void f12();
