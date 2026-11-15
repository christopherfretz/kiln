// C17 6.6p6 (GNU extension; found in unwinder-style code):
// `__builtin_eh_return_data_regno(N)` with an integer constant argument
// is an integer constant expression (clang `getEHDataRegisterNumber`: 0
// and 1 for 0 and 1 on x86-64 and AArch64, -1 otherwise), usable in a
// static assertion, an array bound and an enumerator; `__builtin_dwarf_cfa`
// and `__builtin_extend_pointer` are not constants, so they cannot
// initialize an object with static storage duration. WP-1428.
//@ args: -std=gnu17
//@ expect: compile-fail
//@ strict-diags
//@ diag: 18:18 error "initializer element is not a compile-time constant"
//@ diag: 19:31 error "initializer element is not a compile-time constant"
//@ diag: 20:9 error "argument to '__builtin_eh_return_data_regno' must be a constant integer"
_Static_assert(__builtin_eh_return_data_regno(1) == 1, "");
_Static_assert(__builtin_eh_return_data_regno(7) == -1, "");
int arr[__builtin_eh_return_data_regno(1) + 1];
enum { E = __builtin_eh_return_data_regno(0) };
static void *c = __builtin_dwarf_cfa();
static unsigned long long x = __builtin_extend_pointer(0);
int y = __builtin_eh_return_data_regno(x);
