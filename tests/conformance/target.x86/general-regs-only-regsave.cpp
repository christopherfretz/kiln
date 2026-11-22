// [driver] x86-64 `-mgeneral-regs-only` (WP-1804): without SSE, x87 and
// MMX a `no_caller_saved_registers` function or an interrupt handler may
// call any function, so clang's `-Wexcessive-regsave` (whose text
// suggests the flag) is not given. The target is pinned.
// spec/driver.md "x86-64 target features".
//@ strict-diags
//@ args: --target=x86_64-unknown-linux-gnu -mgeneral-regs-only
void ext(int);
__attribute__((no_caller_saved_registers)) void ncs() { ext(1); }
