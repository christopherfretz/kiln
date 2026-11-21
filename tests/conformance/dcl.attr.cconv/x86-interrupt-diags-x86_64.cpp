// The x86 `interrupt` attribute's checks (clang
// `handleAnyX86InterruptAttr`, `BuildResolvedCallExpr`): on a function
// that is not an instance member function or a static `operator new`,
// `delete`, `()` or `[]` (else a warning drops it), the signature must
// be `void (T *)` or `void (T *, U)` with `U` a 64-bit unsigned type;
// a call naming a handler (through parentheses or `&`) is an error,
// a call through a pointer is not. spec/sema.md §4.5, WP-1745.
//@ expect: compile-fail
//@ args: --target=x86_64-unknown-linux-gnu -ferror-limit=0
//@ strict-diags
//@ diag: 27:28 error "x86-64 'interrupt' attribute only applies to functions that have a 'void' return type"
//@ diag: 28:1 error "x86-64 'interrupt' attribute only applies to functions that have only a pointer parameter optionally followed by an integer parameter"
//@ diag: 29:36 error "x86-64 'interrupt' attribute only applies to functions that have a pointer as the first parameter"
//@ diag: 30:46 error "x86-64 'interrupt' attribute only applies to functions that have a 'unsigned long' type as the second parameter"
//@ diag: 31:1 error "x86-64 'interrupt' attribute only applies to functions that have only a pointer parameter optionally followed by an integer parameter"
//@ diag: 32:36 error "x86-64 'interrupt' attribute only applies to functions that have a pointer as the first parameter"
//@ diag: 33:46 error "x86-64 'interrupt' attribute only applies to functions that have a 'unsigned long' type as the second parameter"
//@ diag: 34:20 error "x86-64 'interrupt' attribute only applies to functions that have only a pointer parameter optionally followed by an integer parameter"
//@ diag: 35:16 warning "'interrupt' attribute only applies to non-K&R-style functions"
//@ diag: 36:27 warning "'interrupt' attribute only applies to non-K&R-style functions"
//@ diag: 37:18 warning "'interrupt' attribute only applies to non-K&R-style functions"
//@ diag: 42:3 error "interrupt service routine cannot be called directly"
//@ diag: 42:10 error "interrupt service routine cannot be called directly"
//@ diag: 42:19 error "interrupt service routine cannot be called directly"
//@ diag: 42:36 error "interrupt service routine cannot be called directly"
struct frame;
__attribute__((interrupt)) int r1(frame *f) { return 0; }
__attribute__((interrupt)) void r2() {}
__attribute__((interrupt)) void r3(int f) {}
__attribute__((interrupt)) void r4(frame *f, int c) {}
__attribute__((interrupt)) void r5(frame *f, unsigned long c, int d) {}
__attribute__((interrupt)) void r6(frame &f) {}
__attribute__((interrupt)) void r7(frame *f, unsigned c) {}
[[gnu::interrupt]] void r8() {}
__attribute__((interrupt)) int v;
struct S { __attribute__((interrupt)) void m(frame *f);
  __attribute__((interrupt)) static void *operator new(unsigned long);
  __attribute__((interrupt)) static void sm(frame *f); };
__attribute__((interrupt)) void ok(frame *f) {}
void (*hp)(frame *) = ok;
void m(frame *f) {
  ok(f); (ok)(f); (&ok)(f); hp(f); S::sm(f);
}
