// [cpp.pragma]: kiln records '#pragma pack(...)' but does not apply it to
// layout yet, so it is reported as unsupported rather than silently ignored.
//@ expect: compile-fail
//@ skip(clang): clang implements '#pragma pack'
//@ strict-diags
//@ diag: 8:9 error "'#pragma pack' is not supported yet"
//@ diag: 10:9 error "'#pragma pack' is not supported yet"
#pragma pack(push, 1)
struct S { char c; int i; };
#pragma pack(pop)
