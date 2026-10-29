// [cpp.pragma]: the '#pragma pack' forms clang diagnoses ('show', an
// alignment other than 1, 2, 4, 8 or 16, 'pop' on an empty stack) are
// reported as unsupported rather than silently ignored.
//@ expect: compile-fail
//@ skip(clang): clang warns and ignores these forms
//@ strict-diags
//@ diag: 9:9 error "'#pragma pack' is not supported yet"
//@ diag: 10:9 error "'#pragma pack' is not supported yet"
#pragma pack(show)
#pragma pack(3)
struct S { char c; int i; };
