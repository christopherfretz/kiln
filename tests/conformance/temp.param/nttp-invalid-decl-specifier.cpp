// [temp.param]/2 (no storage class in a template-parameter),
// [dcl.typedef]/1, [dcl.inline]/1, [dcl.constexpr]/1, [dcl.fct.spec]/1: a
// non-type template parameter's decl-specifiers are checked as clang's
// `CheckValidDeclSpecifiers` does: "invalid declaration specifier in
// template non-type parameter" once per offending specifier, at it, not
// the function-parameter diagnostics. A misplaced `...` is reported
// first (by the parser).
//@ expect: compile-fail
//@ args: -ferror-limit=0
//@ strict-diags
//@ diag: 34:22 error "'...' must immediately precede declared identifier"
//@ diag: 34:10 error "invalid declaration specifier in template non-type parameter"
//@ diag: 35:10 error "invalid declaration specifier in template non-type parameter"
//@ diag: 36:10 error "invalid declaration specifier in template non-type parameter"
//@ diag: 37:10 error "invalid declaration specifier in template non-type parameter"
//@ diag: 38:14 error "invalid declaration specifier in template non-type parameter"
//@ diag: 39:10 error "invalid declaration specifier in template non-type parameter"
//@ diag: 40:10 error "invalid declaration specifier in template non-type parameter"
//@ diag: 41:10 error "invalid declaration specifier in template non-type parameter"
//@ diag: 42:10 error "invalid declaration specifier in template non-type parameter"
//@ diag: 43:10 error "invalid declaration specifier in template non-type parameter"
//@ diag: 44:10 error "invalid declaration specifier in template non-type parameter"
//@ diag: 45:10 error "invalid declaration specifier in template non-type parameter"
//@ diag: 46:10 error "invalid declaration specifier in template non-type parameter"
//@ diag: 47:10 error "invalid declaration specifier in template non-type parameter"
//@ diag: 48:10 error "invalid declaration specifier in template non-type parameter"
//@ diag: 49:10 error "invalid declaration specifier in template non-type parameter"
//@ diag: 49:17 error "invalid declaration specifier in template non-type parameter"
//@ diag: 49:24 error "invalid declaration specifier in template non-type parameter"
//@ diag: 50:10 error "invalid declaration specifier in template non-type parameter"
//@ diag: 51:10 error "invalid declaration specifier in template non-type parameter"
//@ diag: 52:15 error "invalid declaration specifier in template non-type parameter"
//@ diag: 53:10 error "invalid declaration specifier in template non-type parameter"
template<static int a...> struct S1;
template<static int a> struct S2;
template<extern int b> struct S3;
template<static int... c> struct S4;
template<int static d> struct S5;
template<mutable int e> struct S6;
template<thread_local int f> struct S7;
template<register int g> struct S8;
template<constexpr int h> struct S9;
template<inline int i> struct S10;
template<typedef int j> struct S11;
template<virtual int k> struct S12;
template<explicit int l> struct S13;
template<consteval int m> struct S14;
template<constinit int n> struct S15;
template<static inline constexpr int o> struct S16;
template<explicit(true) int p> struct S17;
template<static auto q> struct S18;
template<int, static int r = 0> struct S19;
template<static int s> void f();
