// [dcl.attr.grammar]/5 The x86 `interrupt` attribute (clang 19
// `handleAnyX86InterruptAttr`, `x86_intrcc` with a `byval` frame
// pointer and an optional error code) compiles. spec/types.md §5.4,
// WP-808, WP-1745.
//@ args: --target=x86_64-unknown-linux-gnu
//@ expect: compile-ok
//@ strict-diags
struct frame { unsigned long ip, cs, flags, sp, ss; };
__attribute__((interrupt)) void isr(frame *f) { (void)f; }
__attribute__((interrupt)) void exc(frame *f, unsigned long code) { (void)f; (void)code; }
