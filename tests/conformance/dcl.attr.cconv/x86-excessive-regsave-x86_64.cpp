// `-Wexcessive-regsave` (clang `BuildResolvedCallExpr`): an x86
// interrupt handler or a `no_caller_saved_registers` function that calls
// a function without `no_caller_saved_registers` (or through a pointer)
// warns, as the callee may clobber the x87/MMX/SSE registers the caller
// must preserve; a note names the callee when the call names one.
// spec/sema.md §4.5, WP-1745.
//@ expect: compile-ok
//@ args: --target=x86_64-unknown-linux-gnu
//@ strict-diags
//@ diag: 21:48 warning "interrupt service routine should only call a function with attribute 'no_caller_saved_registers' or be compiled with '-mgeneral-regs-only'"
//@ diag: 17:6 note "'g' declared here"
//@ diag: 21:77 warning "interrupt service routine should only call a function with attribute 'no_caller_saved_registers' or be compiled with '-mgeneral-regs-only'"
//@ diag: 22:55 warning "function with attribute 'no_caller_saved_registers' should only call a function with attribute 'no_caller_saved_registers' or be compiled with '-mgeneral-regs-only'"
//@ diag: 17:6 note "'g' declared here"
struct frame;
typedef unsigned long uword;
void g();
__attribute__((no_caller_saved_registers)) void ncs();
void plain() { g(); }
__attribute__((interrupt)) void h(frame *f) { (void)f; ncs(); }
__attribute__((interrupt)) void h2(frame *f) { g(); ncs(); void (*p)() = g; p(); }
__attribute__((no_caller_saved_registers)) void k() { g(); ncs(); }
