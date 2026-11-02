// [dcl.align]/1, [dcl.spec.general]/1, [dcl.ptr]/1, [dcl.array]/1: an
// attribute-specifier-seq at the end of a decl-specifier-seq, after a
// ptr-operator or after an array declarator appertains to a type, and an
// alignment-specifier is no type attribute. In the middle of a
// decl-specifier-seq, or before a namespace-definition,
// static_assert-declaration or alias-declaration, no attribute list may
// appear ([dcl.pre]/5, [namespace.def]/1).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 19:6 error "'alignas' attribute cannot be applied to types"
//@ diag: 20:13 error "'alignas' attribute cannot be applied to types"
//@ diag: 21:13 error "'alignas' attribute cannot be applied to types"
//@ diag: 22:17 error "'alignas' attribute cannot be applied to types"
//@ diag: 23:5 error "'alignas' attribute cannot be applied to types"
//@ diag: 24:8 error "an attribute list cannot appear here"
//@ diag: 25:1 error "an attribute list cannot appear here"
//@ diag: 26:1 error "an attribute list cannot appear here"
//@ diag: 27:1 error "an attribute list cannot appear here"
char alignas(8) g1;
char arr[3] alignas(2);
typedef int alignas(8) T2;
alignas(8) int *alignas(16) ptr;
int alignas(8) *ptr2;
static alignas(8) int s1;
alignas(8) namespace N {}
alignas(8) static_assert(true);
alignas(8) using T3 = int;
alignas(8) char ok1;
char ok2 alignas(8);
static_assert(__alignof(ok1) == 8 && __alignof(ok2) == 8);
