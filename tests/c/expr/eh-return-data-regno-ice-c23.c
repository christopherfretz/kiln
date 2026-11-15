// C23 6.7.2p5 (GNU extension; found in unwinder-style code): a call of
// `__builtin_eh_return_data_regno` with a constant argument initializes a
// `constexpr` object; the builtins are reported by `__has_builtin`.
// WP-1428.
//@ args: -std=gnu23
//@ expect: compile-ok
//@ strict-diags
#if !__has_builtin(__builtin_eh_return_data_regno) || !__has_builtin(__builtin_dwarf_cfa) \
    || !__has_builtin(__builtin_unwind_init) || !__has_builtin(__builtin_dwarf_sp_column)  \
    || !__has_builtin(__builtin_eh_return) || !__has_builtin(__builtin_extend_pointer)
#error missing unwinder builtin
#endif
constexpr int r0 = __builtin_eh_return_data_regno(0);
constexpr int r1 = __builtin_eh_return_data_regno(r0 + 1);
static_assert(r0 == 0 && r1 == 1 && __builtin_eh_return_data_regno(r1 + 1) == -1);
static_assert(_Generic(__builtin_dwarf_sp_column(), unsigned: 1, default: 0));
