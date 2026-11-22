// [dcl.attr.grammar]/4, [dcl.pre]/5 (GNU extension): every GNU attribute
// before the class-key of a free-standing class or enum declaration is
// ignored with "place it after" -- a known one, an unknown one and one
// with a wrong argument count alike; clang 19's `gcc_struct` is unknown
// there and gets the same warning (spec/parse.md §6.2 "Argument table",
// WP-1882). Verified against clang 19 and 22.
//@ requires: clang<22
//@ expect: compile-ok
//@ strict-diags
//@ diag: 34:16 warning "attribute 'gcc_struct' is ignored, place it after"
//@ diag: 35:23 warning "unknown attribute 'gcc_struct' ignored"
//@ diag: 36:16 warning "unknown attribute 'gcc_struct' ignored"
//@ diag: 37:16 warning "attribute 'ms_struct' is ignored, place it after"
//@ diag: 38:16 warning "attribute 'gcc_struct' is ignored, place it after"
//@ diag: 39:16 warning "attribute 'gcc_struct' is ignored, place it after"
//@ diag: 40:37 warning "unknown attribute 'gcc_struct' ignored"
//@ diag: 41:16 warning "attribute 'kiln_nosuch' is ignored, place it after"
//@ diag: 42:16 warning "attribute 'kiln_nosuch' is ignored, place it after"
//@ diag: 43:16 warning "attribute 'long_call' is ignored, place it after"
//@ diag: 44:27 warning "attribute 'kiln_nosuch' is ignored, place it after"
//@ diag: 44:77 warning "attribute 'packed' is ignored, place it after"
//@ diag: 45:16 warning "attribute 'kiln_nosuch' is ignored, place it after"
//@ diag: 46:16 warning "attribute 'packed' is ignored, place it after"
//@ diag: 47:16 warning "attribute 'kiln_nosuch' is ignored, place it after"
//@ diag: 48:27 warning "attribute 'kiln_nosuch' is ignored, place it after"
//@ diag: 48:77 warning "attribute 'packed' is ignored, place it after"
//@ diag: 49:16 warning "unknown attribute 'kiln_nosuch' ignored"
//@ diag: 50:16 warning "unknown attribute 'kiln_nosuch' ignored"
//@ diag: 51:1 warning "'const' is not permitted on a declaration of a type"
//@ diag: 51:22 warning "attribute 'kiln_nosuch' is ignored, place it after"
//@ diag: 52:32 warning "attribute 'kiln_nosuch' is ignored, place it after"
//@ diag: 53:16 warning "attribute 'kiln_nosuch' is ignored, place it after"

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
