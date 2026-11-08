// clang 22 gcc_struct (WP-809; GCCStructAttr): the default layout, overriding
// #pragma ms_struct and an ms_struct on any other declaration; with ms_struct
// on one declaration the later of the two is an error and dropped.
//@ expect: compile-fail
//@ requires: clang>=22
//@ strict-diags
//@ diag: 31:51 error "'ms_struct' and 'gcc_struct' attributes are not compatible"
//@ diag: 31:23 note "conflicting attribute is here"
//@ diag: 32:57 error "'ms_struct' and 'gcc_struct' attributes are not compatible"
//@ diag: 32:23 note "conflicting attribute is here"
//@ diag: 33:55 error "'gcc_struct' and 'ms_struct' attributes are not compatible"
//@ diag: 33:23 note "conflicting attribute is here"
//@ diag: 34:34 error "'gcc_struct' and 'ms_struct' attributes are not compatible"
//@ diag: 34:23 note "conflicting attribute is here"
//@ diag: 40:50 warning "width of bit-field 'x' (40 bits) exceeds the width of its type; value will be truncated to 32 bits"
//@ diag: 41:20 warning "attribute only applies to structs, unions, and classes"
//@ diag: 42:23 error "attribute takes no arguments"
//@ diag: 44:23 warning "attribute declaration must precede definition"
//@ diag: 43:8 note "previous definition is here"
#define BF { char a : 3; int b : 4; }
#pragma ms_struct on
struct __attribute__((gcc_struct)) S BF;
struct T BF __attribute__((gcc_struct));
struct P BF;
#pragma ms_struct off
struct __attribute__((gcc_struct, gcc_struct)) U BF;
struct __attribute__((gcc_struct)) V;
struct __attribute__((ms_struct)) V BF;
struct __attribute__((ms_struct)) X;
struct __attribute__((gcc_struct)) X BF;
struct __attribute__((gcc_struct)) __attribute__((ms_struct)) W BF;
struct __attribute__((gcc_struct)) W2 BF __attribute__((ms_struct));
struct __attribute__((ms_struct)) Y BF __attribute__((gcc_struct));
struct __attribute__((ms_struct, gcc_struct)) Z BF;
struct [[gnu::gcc_struct]] G BF;
static_assert(sizeof(S) == 4 && sizeof(T) == 4 && sizeof(P) == 8 && sizeof(U) == 4);
static_assert(sizeof(V) == 4 && sizeof(X) == 4 && sizeof(W) == 4 && sizeof(W2) == 4);
static_assert(sizeof(Y) == 8 && sizeof(Z) == 8 && sizeof(G) == 4);
struct __attribute__((gcc_struct)) B1 { int x; };
struct __attribute__((gcc_struct)) D2 : B1 { int x : 40; };
int __attribute__((gcc_struct)) v;
struct __attribute__((gcc_struct(1))) C { int x; };
struct E { int x; };
struct __attribute__((gcc_struct)) E;
