// [cpp.pragma]/1: the '#pragma pack' forms clang ignores or reports get clang's
// warnings: 'show', an alignment other than 0, 1, 2, 4, 8 or 16, 'pop' on an
// empty stack or with both a label and an alignment, an unknown action, a
// malformed argument list, extra tokens, and a push never popped at the end of
// the file. WP-1744.
//@ expect: compile-ok
//@ strict-diags
//@ diag: 29:9 warning "value of #pragma pack(show) == 8"
//@ diag: 30:9 warning "expected #pragma pack parameter to be '1', '2', '4', '8', or '16'"
//@ diag: 31:9 warning "#pragma pack(pop, ...) failed: stack empty"
//@ diag: 32:9 warning "#pragma pack(pop, ...) failed: stack empty"
//@ diag: 33:14 warning "unknown action for '#pragma pack' - ignored"
//@ diag: 35:9 warning "value of #pragma pack(show) == 2"
//@ diag: 36:9 warning "specifying both a name and alignment to 'pop' is undefined"
//@ diag: 37:9 warning "value of #pragma pack(show) == 4"
//@ diag: 38:9 warning "expected #pragma pack parameter to be '1', '2', '4', '8', or '16'"
//@ diag: 41:9 warning "value of #pragma pack(show) == 1"
//@ diag: 42:9 warning "#pragma pack(pop, ...) failed: stack empty"
//@ diag: 43:15 warning "missing ')' after '#pragma pack' - ignoring"
//@ diag: 45:23 warning "expected integer or identifier in '#pragma pack' - ignored"
//@ diag: 46:20 warning "missing ')' after '#pragma pack' - ignoring"
//@ diag: 47:17 warning "extra tokens at end of '#pragma pack' - ignored"
//@ diag: 49:9 warning "value of #pragma pack(show) == 8"
//@ diag: 50:14 warning "missing ')' after '#pragma pack' - ignoring"
//@ diag: 51:19 warning "missing ')' after '#pragma pack' - ignoring"
//@ diag: 52:9 warning "expected #pragma pack parameter to be '1', '2', '4', '8', or '16'"
//@ diag: 53:9 warning "expected #pragma pack parameter to be '1', '2', '4', '8', or '16'"
//@ diag: 48:9 warning "unterminated '#pragma pack (push, ...)' at end of file"
#pragma pack(show)
#pragma pack(3)
#pragma pack(pop)
#pragma pack(pop, foo)
#pragma pack(bogus)
#pragma pack(push, l, 2)
#pragma pack(show)
#pragma pack(pop, l, 4)
#pragma pack(show)
#pragma pack(push, 32)
#pragma pack(push, l2)
#pragma pack(pop, 1)
#pragma pack(show)
#pragma pack(pop, l3)
#pragma pack(4
#pragma pack()
#pragma pack(push, x, y)
#pragma pack(pop, 2, 2)
#pragma pack(1) extra
#pragma pack(push, 0)
#pragma pack(show)
#pragma pack(,)
#pragma pack(push 2)
#pragma pack(push, 3)
#pragma pack(pop, 3)
struct S { char c; int i; };
static_assert(sizeof(S) == 8);
