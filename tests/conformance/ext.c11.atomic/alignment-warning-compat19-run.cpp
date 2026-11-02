//@ expect: run
//@ requires: target-aarch64
//@ requires: clang<22
//@ args: -latomic
//@ strict-diags
//@ diag: 13:3 warning "misaligned atomic operation may incur significant performance penalty; the expected alignment (32 bytes) exceeds the actual alignment (1 bytes)"
//@ diag: 13:3 warning "large atomic operation may incur significant performance penalty; the access size (32 bytes) exceeds the max lock-free size (16  bytes)"
//@ stdout: x
// WP-355: clang 19's -Watomic-alignment text has a doubled space before
// "bytes)" (clang 22 has one; alignment-warning-run.cpp).
extern "C" int printf(const char *, ...);
struct B { char c[32]; }; B gb; int main() { B t = {{'x'}};
  __atomic_store(&gb, &t, 5);
  printf("%c\n", gb.c[0]);
}
