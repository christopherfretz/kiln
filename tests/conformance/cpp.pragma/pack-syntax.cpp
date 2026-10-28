// [cpp.pragma]: '#pragma pack' without '(' is ignored with a warning.
//@ expect: compile-ok
//@ strict-diags
//@ diag: 7:14 warning "missing '(' after '#pragma pack' - ignoring"
#define PACK pack
struct S { char c; int i; };
#pragma pack 4
#pragma PACK
static_assert(sizeof(S) == 8);
