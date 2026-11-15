// The GNU unwinder builtins in C on AArch64 (found in unwinder-style
// code; clang CGBuiltin EmitBuiltinExpr): `__builtin_dwarf_cfa()` is
// `llvm.eh.dwarf.cfa(i32 0)`, `__builtin_sponentry()` (an AArch64 target
// builtin) `llvm.sponentry.p0`, `__builtin_unwind_init()`
// `llvm.eh.unwind.init`, `__builtin_eh_return` `llvm.eh.return.i64` of
// the offset converted to size_t and then `unreachable`;
// `__builtin_eh_return_data_regno` (0 -> 0, 1 -> 1, else -1) and
// `__builtin_dwarf_sp_column()` (31, `sp`) are target constants. WP-1428.
// The clang 22 IR; clang 19's is the -19 sibling.
//@ requires: clang>=22
//@ args: --target=aarch64-unknown-linux-gnu -std=gnu17
//@ expect: ir
void *cfa(void) { return __builtin_dwarf_cfa(); }
void *spe(void) { return __builtin_sponentry(); }
void ui(void) { __builtin_unwind_init(); }
int regno0(void) { return __builtin_eh_return_data_regno(0); }
int regno1(void) { return __builtin_eh_return_data_regno(1); }
int regno2(void) { return __builtin_eh_return_data_regno(2); }
unsigned spcol(void) { return __builtin_dwarf_sp_column(); }
void ehret(long off, void *h) { __builtin_eh_return(off, h); }
void ehret_int(int off, void *h) { __builtin_eh_return(off, h); }
unsigned long long ext(void *p) { return __builtin_extend_pointer(p); }
