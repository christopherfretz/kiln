//@ expect: run
//@ requires: clang>=23
//@ oracle: none
//@ strict-diags
//@ args: -Rauto-var-init
//@ stdout: at 0
//@ diag: 17:26 remark "erroneous value of variable 'junk' not written: its size (8192 bytes) exceeds -ftrivial-auto-var-init-max-size=4096"
//@ diag: 29:7 remark "erroneous value of variable 'b' not written: its size (4100 bytes) exceeds -ftrivial-auto-var-init-max-size=4096"
// [basic.indet]/1 (C++26, P2795R5), spec/codegen.md §7q: kiln writes the
// erroneous value (zero bytes) of an automatic variable of at most 4096
// bytes; a larger one is left unwritten (its value is still erroneous,
// whatever bytes the stack holds) and `-Rauto-var-init` says so. The
// limit is `-ftrivial-auto-var-init-max-size`'s default. clang 22 has no
// erroneous values (`oracle: none`).
extern "C" int printf(const char *, ...);
[[gnu::noinline]] void dirty() {
  volatile unsigned char junk[8192];
  for (int k = 0; k < 8192; ++k)
    junk[k] = 0x11;
}
[[gnu::noinline]] int at() {
  int a[1024];
  int s = 0;
  for (int k = 0; k < 1024; ++k)
    s |= a[k];
  return s;
}
[[gnu::noinline]] void above(void (*sink)(int *)) {
  int b[1025];
  sink(b);
}
static void ignore(int *) {}
int main() {
  dirty();
  printf("at %d\n", at());
  above(ignore);
}
