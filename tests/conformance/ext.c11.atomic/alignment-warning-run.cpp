//@ expect: run
//@ args: -latomic
//@ strict-diags
//@ diag: 23:3 warning "misaligned atomic operation may incur significant performance penalty; the expected alignment (32 bytes) exceeds the actual alignment (1 bytes)"
//@ diag: 23:3 warning "large atomic operation may incur significant performance penalty; the access size (32 bytes) exceeds the max lock-free size (16  bytes)"
//@ diag: 24:3 warning "misaligned atomic operation may incur significant performance penalty; the expected alignment (32 bytes) exceeds the actual alignment (1 bytes)"
//@ diag: 24:3 warning "large atomic operation may incur significant performance penalty; the access size (32 bytes) exceeds the max lock-free size (16  bytes)"
//@ diag: 26:3 warning "misaligned atomic operation may incur significant performance penalty; the expected alignment (3 bytes) exceeds the actual alignment (1 bytes)"
//@ diag: 27:9 warning "misaligned atomic operation may incur significant performance penalty; the expected alignment (32 bytes) exceeds the actual alignment (1 bytes)"
//@ diag: 27:9 warning "large atomic operation may incur significant performance penalty; the access size (32 bytes) exceeds the max lock-free size (16  bytes)"
//@ diag: 30:9 warning "misaligned atomic operation may incur significant performance penalty; the expected alignment (32 bytes) exceeds the actual alignment (1 bytes)"
//@ diag: 30:9 warning "large atomic operation may incur significant performance penalty; the access size (32 bytes) exceeds the max lock-free size (16  bytes)"
//@ stdout: x y z
// WP-165: clang's -Watomic-alignment, reported by codegen for the atomic
// builtins (on aarch64, whose max inline atomic width is 16 bytes);
// __c11_atomic_init and _Atomic lvalue accesses report nothing.
extern "C" int printf(const char *, ...);
struct B { char c[32]; };
struct S3 { char c[3]; };
B gb; S3 g3; _Atomic(B) ab; _Atomic(int) ai;
int main() {
  B t = {{'x'}};
  __atomic_store(&gb, &t, __ATOMIC_SEQ_CST);
  __atomic_load(&gb, &t, __ATOMIC_SEQ_CST);
  S3 u = {{'y'}};
  __atomic_store(&g3, &u, 5);
  B r = __c11_atomic_exchange(&ab, t, 5);
  __c11_atomic_init(&ai, 1);
  ab = t;
  B s = __c11_atomic_load(&ab, 5);
  printf("%c %c %c\n", s.c[0], g3.c[0], (char)('z' + r.c[0]));
}
