// [dcl.attr.grammar]/4, [dcl.pre]/5 (GNU extension): every GNU attribute
// before the class-key of a free-standing class or enum declaration is
// ignored with "place it after" -- a known one, an unknown one and one
// with a wrong argument count alike; clang 19's `gcc_struct` is unknown
// there and gets the same warning (spec/parse.md §6.2 "Argument table",
// WP-1882). Verified against clang 19 and 22.
//@ requires: clang>=22
//@ expect: compile-fail
//@ strict-diags
//@ diag: 32:16 warning "attribute 'gcc_struct' is ignored, place it after"
//@ diag: 34:16 warning "'gcc_struct' attribute only applies to structs, unions, and classes"
//@ diag: 35:16 warning "attribute 'ms_struct' is ignored, place it after"
//@ diag: 36:16 warning "attribute 'gcc_struct' is ignored, place it after"
//@ diag: 37:27 error "use of undeclared identifier 'x'"
//@ diag: 39:16 warning "attribute 'kiln_nosuch' is ignored, place it after"
//@ diag: 40:16 warning "attribute 'kiln_nosuch' is ignored, place it after"
//@ diag: 41:16 warning "attribute 'long_call' is ignored, place it after"
//@ diag: 42:27 warning "attribute 'kiln_nosuch' is ignored, place it after"
//@ diag: 42:77 warning "attribute 'packed' is ignored, place it after"
//@ diag: 43:16 warning "attribute 'kiln_nosuch' is ignored, place it after"
//@ diag: 44:16 warning "attribute 'packed' is ignored, place it after"
//@ diag: 45:16 warning "attribute 'kiln_nosuch' is ignored, place it after"
//@ diag: 46:27 warning "attribute 'kiln_nosuch' is ignored, place it after"
//@ diag: 46:77 warning "attribute 'packed' is ignored, place it after"
//@ diag: 47:16 warning "unknown attribute 'kiln_nosuch' ignored"
//@ diag: 48:16 warning "unknown attribute 'kiln_nosuch' ignored"
//@ diag: 49:1 warning "'const' is not permitted on a declaration of a type"
//@ diag: 49:22 warning "attribute 'kiln_nosuch' is ignored, place it after"
//@ diag: 50:32 warning "attribute 'kiln_nosuch' is ignored, place it after"
//@ diag: 51:16 warning "attribute 'kiln_nosuch' is ignored, place it after"

__attribute__((gcc_struct)) struct S1 { int a; };
struct __attribute__((gcc_struct)) S2 { int a; };
__attribute__((gcc_struct)) int v;
__attribute__((ms_struct)) struct S3 { int a; };
__attribute__((gcc_struct(1))) struct S4 { int a; };
__attribute__((gcc_struct(x))) struct S5 { int a; };
struct S6 { int a; } __attribute__((gcc_struct));
__attribute__((kiln_nosuch)) struct S8 { int a; };
__attribute__((kiln_nosuch(1))) struct S9 { int a; };
__attribute__((long_call)) struct S10 { int a; };
struct O { __attribute__((kiln_nosuch)) struct T { int a; }; __attribute__((packed)) struct T2 { int a; }; };
__attribute__((kiln_nosuch)) struct Fwd;
__attribute__((packed)) struct Fwd2;
__attribute__((kiln_nosuch)) enum E { e1 };
void f() { __attribute__((kiln_nosuch)) struct L { int a; }; __attribute__((packed)) struct L2 { int a; }; }
__attribute__((kiln_nosuch)) typedef struct S23 { int a; } T23;
__attribute__((kiln_nosuch)) struct S24 { int a; } s24;
const __attribute__((kiln_nosuch)) struct S25 { int a; };
template<class> __attribute__((kiln_nosuch)) struct S26 { int a; };
__attribute__((kiln_nosuch)) union U7 { int a; };
