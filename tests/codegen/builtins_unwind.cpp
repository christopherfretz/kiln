// WP-1428: the unwinder builtins (clang CGBuiltin EmitBuiltinExpr; found
// in unwinder-style code): `__builtin_dwarf_cfa()` is
// `llvm.eh.dwarf.cfa(i32 0)`, `__builtin_unwind_init()`
// `llvm.eh.unwind.init` (a plain call, no invoke), `__builtin_eh_return`
// `llvm.eh.return.i64` of the offset converted to size_t, then
// `unreachable`; `__builtin_eh_return_data_regno(N)` (0 -> 0, 1 -> 1,
// else -1) and `__builtin_dwarf_sp_column()` (7 on x86-64, 31 on
// AArch64) are target constants; `__builtin_extend_pointer` is a
// `ptrtoint` to i64.

struct D { ~D(); };
void *cfa() { return __builtin_dwarf_cfa(); }
void ui() { __builtin_unwind_init(); }
void ui_dtor() { D d; __builtin_unwind_init(); }
int regno0() { return __builtin_eh_return_data_regno(0); }
int regno1() { return __builtin_eh_return_data_regno(1); }
int regno2() { return __builtin_eh_return_data_regno(2); }
int regno_neg() { return __builtin_eh_return_data_regno(-1); }
unsigned spcol() { return __builtin_dwarf_sp_column(); }
void ehret(long off, void *h) { __builtin_eh_return(off, h); }
void ehret_int(int off, void *h) { __builtin_eh_return(off, h); }
int ehret_dtor(long off, void *h) { D d; __builtin_eh_return(off, h); }
unsigned long long ext(void *p) { return __builtin_extend_pointer(p); }
template <int N> int regno_t() { return __builtin_eh_return_data_regno(N); }
int use_t() { return regno_t<1>() + regno_t<3>(); }
