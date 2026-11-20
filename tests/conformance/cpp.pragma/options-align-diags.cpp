// [cpp.pragma]/1: '#pragma options align' and '#pragma align' with clang's
// warnings for the forms it ignores ('reset' with an empty stack and the default
// value, an unknown option, a missing '=' or 'align', extra tokens). WP-1744.
//@ expect: compile-ok
//@ strict-diags
//@ diag: 26:9 warning "#pragma options align=reset failed: stack empty"
//@ diag: 27:23 warning "invalid alignment option in '#pragma options align' - ignored"
//@ diag: 28:22 warning "expected '=' following '#pragma options align' - ignored"
//@ diag: 29:17 warning "expected 'align' following '#pragma options' - ignored"
//@ diag: 30:14 warning "expected '=' following '#pragma align' - ignored"
//@ diag: 31:22 warning "extra tokens at end of '#pragma align' - ignored"
#pragma options align=packed
struct A { char c; int i; };
#pragma options align=reset
struct B { char c; int i; };
#pragma align=packed
struct C { char c; int i; };
#pragma align=reset
#pragma options align=natural
struct D { char c; double d; };
#pragma options align=power
struct E { char c; double d; };
#pragma options align=reset
#pragma options align=reset
struct F { char c; int i; };
#pragma options align=reset
#pragma options align=bogus
#pragma options align
#pragma options foo
#pragma align
#pragma align=packed extra
static_assert(sizeof(A)==5);
static_assert(sizeof(B)==8);
static_assert(sizeof(C)==5);
static_assert(sizeof(D) == 16 && sizeof(E) == 16 && sizeof(F) == 8);
