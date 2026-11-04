//@ expect: run
//@ diag: 16:34 warning "taking address of packed member 'l' of class or structure 'P' may result in an unaligned pointer value"
//@ diag: 16:12 warning "__sync builtin operation must have natural alignment (consider using __atomic)"
//@ stdout: 5 6 9
// clang `CodeGenFunction::EmitBuiltinExpr`'s `__sync` lowering
// (`CheckAtomicAlignment`): an operand pointer whose known alignment is
// below the operation's size warns -Wsync-alignment ("__sync builtin
// operation must have natural alignment") at the call, and the atomic
// instruction still uses the natural alignment. The shape is a counter in
// a packed wire-format struct.
extern "C" int printf(const char *, ...);
struct __attribute__((packed, aligned(4))) P { char c[4]; int i; long l; };
int main() {
  P p = {{}, 5, 8};
  int a = __sync_fetch_and_add(&p.i, 1);  // 4-aligned: no warning
  long b = __sync_add_and_fetch(&p.l, 1); // only 4-aligned
  printf("%d %d %ld\n", a, p.i, b);
}
