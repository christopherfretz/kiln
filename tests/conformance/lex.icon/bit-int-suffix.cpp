// [lex.icon]/1: C++ has no `wb`/`uwb` suffix (C23's bit-precise integer
// constants), so a literal of a `_BitInt` type, also one wider than 128
// bits, cannot be spelled; the suffix is invalid (clang).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 9:11 error "invalid suffix 'wb' on integer constant"
//@ diag: 10:46 error "invalid suffix 'wb' on integer constant"
//@ diag: 11:11 error "invalid suffix 'uwb' on integer constant"
auto a = 1wb;
auto b = 0xffffffffffffffffffffffffffffffffffwb;
auto c = 5uwb;
