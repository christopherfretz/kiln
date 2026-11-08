// GNU ms_struct and #pragma ms_struct (WP-809; clang MSStructAttr): classes only,
// no arguments; with bases or virtual functions an error (default-error
// -Wincompatible-ms-struct), again per instantiation; a bit-field wider than
// its type's storage is an error once the class is known to be ms_struct
// (not for the trailing form, which only warns); added after the definition
// it is ignored; a malformed pragma is ignored with a warning.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 39:43 error "width of bit-field 'x' (33 bits) exceeds the size of its type (32 bits)"
//@ diag: 40:16 warning "width of bit-field 'x' (33 bits) exceeds the width of its type; value will be truncated to 32 bits"
//@ diag: 41:20 warning "attribute only applies to structs, unions, and classes"
//@ diag: 42:21 warning "attribute only applies to structs, unions, and classes"
//@ diag: 43:23 error "attribute takes no arguments"
//@ diag: 44:35 error "ms_struct may not produce Microsoft-compatible layouts for classes with base classes or virtual functions"
//@ diag: 45:35 error "ms_struct may not produce Microsoft-compatible layouts for classes with base classes or virtual functions"
//@ diag: 46:21 warning "attribute only applies to structs, unions, and classes"
//@ diag: 48:53 error "ms_struct may not produce Microsoft-compatible layouts for classes with base classes or virtual functions"
//@ diag: 48:53 error "ms_struct may not produce Microsoft-compatible layouts for classes with base classes or virtual functions"
//@ diag: 49:6 note "in instantiation of template class 'J<A>' requested here"
//@ diag: 50:43 error "width of bit-field 'x' (65 bits) exceeds the size of its type (64 bits)"
//@ diag: 52:16 error "width of bit-field 'x' (40 bits) exceeds the size of its type (32 bits)"
//@ diag: 54:23 warning "attribute declaration must precede definition"
//@ diag: 53:8 note "previous definition is here"
//@ diag: 55:18 warning "incorrect use of '#pragma ms_struct on|off' - ignored"
//@ diag: 56:19 warning "incorrect use of '#pragma ms_struct on|off' - ignored"
//@ diag: 57:22 warning "extra tokens at end of '#pragma ms_struct' - ignored"
//@ diag: 58:19 warning "incorrect use of '#pragma ms_struct on|off' - ignored"
//@ diag: 60:16 error "width of bit-field 'x' (40 bits) exceeds the size of its type (32 bits)"
//@ diag: 62:16 warning "width of bit-field 'x' (40 bits) exceeds the width of its type; value will be truncated to 32 bits"
//@ diag: 63:43 error "width of anonymous bit-field (33 bits) exceeds the size of its type (32 bits)"
//@ diag: 63:54 error "width of bit-field 'b' (9 bits) exceeds the size of its type (8 bits)"
//@ diag: 64:31 warning "attribute only applies to structs, unions, and classes"
//@ diag: 65:3 warning "attribute only applies to structs, unions, and classes"
//@ diag: 66:37 error "width of bit-field 's' (17 bits) exceeds the size of its type (16 bits)"
//@ diag: 68:36 warning "attribute only applies to structs, unions, and classes"
//@ diag: 68:70 warning "attribute only applies to structs, unions, and classes"
//@ diag: 69:53 error "ms_struct may not produce Microsoft-compatible layouts for classes with base classes or virtual functions"
//@ diag: 71:8 error "ms_struct may not produce Microsoft-compatible layouts for classes with base classes or virtual functions"
struct __attribute__((ms_struct)) A { int x : 33; };
struct B { int x : 33; } __attribute__((ms_struct));
int __attribute__((ms_struct)) v;
void __attribute__((ms_struct)) f();
struct __attribute__((ms_struct(1))) C { int x; };
struct __attribute__((ms_struct)) D : A { int y; };
struct __attribute__((ms_struct)) E { virtual void g(); };
enum __attribute__((ms_struct)) H { h };
typedef struct { char c : 2; } __attribute__((ms_struct)) I;
template<class T> struct __attribute__((ms_struct)) J : T { };
J<A> ja;
union __attribute__((ms_struct)) K { long x : 65; };
struct __attribute__((ms_struct)) L;
struct L { int x : 40; };
struct M { char x; };
struct __attribute__((ms_struct)) M;
#pragma ms_struct
#pragma ms_struct foo
#pragma ms_struct on x
#pragma ms_struct (on)
#pragma ms_struct on
struct N { int x : 40; };
#pragma ms_struct reset
struct O { int x : 40; };
struct __attribute__((ms_struct)) P { int : 33; bool b : 9; char c : 8; };
typedef int TI __attribute__((ms_struct));
[[gnu::ms_struct]] int w;
struct [[gnu::ms_struct]] Q { short s : 17; };
struct __attribute__((ms_struct)) R { int x; } __attribute__((ms_struct));
struct S { void f() __attribute__((ms_struct)); int m __attribute__((ms_struct)); };
template<class T> struct __attribute__((ms_struct)) V { virtual void g(); };
#pragma ms_struct on
struct W : A {};
#pragma ms_struct off
