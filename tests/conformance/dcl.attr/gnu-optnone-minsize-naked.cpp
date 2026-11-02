// GNU `optnone` / `minsize` / `naked` (WP-405; clang
// handleOptimizeNoneAttr, handleMinSizeAttr, CheckNakedParmReference,
// ActOnFinishFunctionBody): each takes no arguments and applies to
// functions only (`minsize` on a variable is an error, the others warn);
// `optnone` overrides `always_inline` and `minsize` on the same or another
// declaration ("'X' attribute ignored" at the loser, a note at `optnone`);
// a naked function body holds only asm statements (null statements and
// local register variables allowed) and its asm operands cannot name a
// parameter or `this`; falling off a naked function is not diagnosed.
// [dcl.attr.grammar]/5 (implementation-defined attributes).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 39:16 warning "'optnone' attribute only applies to functions and Objective-C methods"
//@ diag: 40:16 warning "'naked' attribute only applies to functions"
//@ diag: 41:16 error "'minsize' attribute only applies to functions and Objective-C methods"
//@ diag: 42:16 warning "'always_inline' attribute ignored"
//@ diag: 42:31 note "conflicting attribute is here"
//@ diag: 43:25 warning "'always_inline' attribute ignored"
//@ diag: 43:16 note "conflicting attribute is here"
//@ diag: 44:25 warning "'minsize' attribute ignored"
//@ diag: 44:16 note "conflicting attribute is here"
//@ diag: 45:16 warning "'minsize' attribute ignored"
//@ diag: 46:16 note "conflicting attribute is here"
//@ diag: 48:16 warning "'always_inline' attribute ignored"
//@ diag: 47:16 note "conflicting attribute is here"
//@ diag: 49:16 error "'optnone' attribute takes no arguments"
//@ diag: 50:16 error "'minsize' attribute takes no arguments"
//@ diag: 51:16 error "'naked' attribute takes no arguments"
//@ diag: 56:36 error "non-ASM statement in naked function is not supported"
//@ diag: 56:16 note "attribute is here"
//@ diag: 57:48 error "non-ASM statement in naked function is not supported"
//@ diag: 57:16 note "attribute is here"
//@ diag: 58:57 error "parameter references not allowed in naked functions"
//@ diag: 58:16 note "attribute is here"
//@ diag: 62:51 error "'this' pointer references not allowed in naked functions"
//@ diag: 62:18 note "attribute is here"
//@ diag: 66:13 error "non-ASM statement in naked function is not supported"
//@ diag: 65:16 note "attribute is here"
__attribute__((optnone)) int v1;
__attribute__((naked)) int v2;
__attribute__((minsize)) int v3;
__attribute__((always_inline, optnone)) inline void a1() {}
__attribute__((optnone, always_inline)) inline void a2() {}
__attribute__((optnone, minsize)) void a3() {}
__attribute__((minsize)) void a4();
__attribute__((optnone)) void a4() {}
__attribute__((optnone)) void a5();
__attribute__((always_inline)) inline void a5() {}
__attribute__((optnone(1))) void a6() {}
__attribute__((minsize(1))) void a7() {}
__attribute__((naked(1))) void a8() {}
[[clang::optnone]] void a9() {}
[[clang::minsize]] void a10() {}
[[gnu::naked]] void a11() { asm("nop"); }
__attribute__((naked)) void n1() { asm("nop"); ; }
__attribute__((naked)) void n2() { int x = 0; }
__attribute__((naked)) void n3() { asm("nop"); { asm("nop"); } }
__attribute__((naked)) int n4(int p) { asm("nop" :: "r"(p)); }
__attribute__((naked)) int n5() { }
struct S {
  int m;
  __attribute__((naked)) void f() { asm("" :: "r"(m)); }
};
__attribute__((naked)) [[noreturn]] void n7() { asm("nop"); }
__attribute__((naked)) void n8();
void n8() { return; }
