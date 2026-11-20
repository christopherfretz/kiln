// [cpp.pragma]/1: '#pragma options align=k' and '#pragma align=k' share
// '#pragma pack''s stack (clang AlignPackStack): 'packed' packs the classes
// defined while it is in effect, 'natural' and 'power' push the default
// layout, 'reset' pops; '#pragma pack(show)' prints the value. WP-1744.
//@ expect: compile-ok
//@ strict-diags
//@ diag: 17:9 warning "value of #pragma pack(show) == 1"
//@ diag: 20:9 warning "value of #pragma pack(show) == 8"
//@ diag: 23:9 warning "value of #pragma pack(show) == 8"
//@ diag: 25:9 warning "value of #pragma pack(show) == 8"
//@ diag: 29:9 warning "value of #pragma pack(show) == 8"
//@ diag: 32:9 warning "value of #pragma pack(show) == 8"
//@ diag: 37:9 warning "#pragma pack(pop, ...) failed: stack empty"
//@ diag: 42:9 warning "value of #pragma pack(show) == 4"
//@ diag: 43:9 warning "#pragma pack(pop, ...) failed: stack empty"
#pragma options align=packed
#pragma pack(show)
struct A { char c; int i; };
#pragma pack(pop)
#pragma pack(show)
#pragma pack(2)
#pragma options align=reset
#pragma pack(show)
#pragma options align=natural
#pragma pack(show)
struct B { char c; double d; };
#pragma pack(push, 1)
#pragma options align=reset
#pragma pack(show)
#pragma options align=reset
#pragma align=power
#pragma pack(show)
#pragma align=packed
struct C { char c; double d; struct { char x; int y; } in; };
#pragma align=reset
#pragma align=reset
#pragma pack(pop)
#pragma pack(4)
#pragma options align=natural
struct D { char c; double d; };
#pragma options align=reset
#pragma pack(show)
#pragma pack(pop)
static_assert(sizeof(A)==5);
static_assert(alignof(C)==1 && sizeof(C)==14);
static_assert(sizeof(D)==16);
static_assert(sizeof(B)==16);
