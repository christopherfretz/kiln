// `__builtin_eh_return_data_regno(N)` (GNU; found in unwinder-style code)
// is a core constant expression ([expr.const]): the DWARF number of the
// Nth exception-handling data register, clang `getEHDataRegisterNumber`,
// 0 and 1 for 0 and 1 on x86-64 and AArch64 and -1 for any other N. It
// works in templates (a value-dependent argument is checked at
// instantiation) and `__has_builtin` reports it and its neighbours.
// The types and exception specifications are clang's (Builtins.td).
// WP-1428.
//@ expect: compile-ok
//@ strict-diags
#if !__has_builtin(__builtin_eh_return_data_regno) || !__has_builtin(__builtin_dwarf_cfa) \
    || !__has_builtin(__builtin_unwind_init) || !__has_builtin(__builtin_dwarf_sp_column)  \
    || !__has_builtin(__builtin_eh_return) || !__has_builtin(__builtin_extend_pointer)
#error missing unwinder builtin
#endif
static_assert(__builtin_eh_return_data_regno(0) == 0);
static_assert(__builtin_eh_return_data_regno(1) == 1);
static_assert(__builtin_eh_return_data_regno(2) == -1);
static_assert(__builtin_eh_return_data_regno(-5) == -1);
template <int N> constexpr int r() { return __builtin_eh_return_data_regno(N); }
static_assert(r<0>() == 0 && r<1>() == 1 && r<7>() == -1);
constexpr int k = 1;
int arr[__builtin_eh_return_data_regno(k) + 1];
static_assert(sizeof arr == 2 * sizeof(int));
static_assert(__is_same(decltype(__builtin_eh_return_data_regno(0)), int));
static_assert(__is_same(decltype(__builtin_dwarf_sp_column()), unsigned));
static_assert(__is_same(decltype(__builtin_dwarf_cfa()), void *));
static_assert(__is_same(decltype(__builtin_unwind_init()), void));
static_assert(__is_same(decltype(__builtin_eh_return(0, 0)), void));
static_assert(__is_same(decltype(__builtin_extend_pointer(nullptr)), unsigned long long));
static_assert(noexcept(__builtin_dwarf_cfa()) && noexcept(__builtin_dwarf_sp_column()));
static_assert(noexcept(__builtin_eh_return_data_regno(0)));
static_assert(noexcept(__builtin_extend_pointer(nullptr)));
static_assert(!noexcept(__builtin_unwind_init()) && !noexcept(__builtin_eh_return(0, 0)));
