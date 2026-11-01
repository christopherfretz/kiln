// GNU __builtin_alloca and __builtin_alloca_with_align (clang
// EmitBuiltinExpr): a dynamic `alloca i8` where the call is, aligned to
// __BIGGEST_ALIGNMENT__ or to the requested alignment in bits.
void use(void *);
void f1(unsigned long n) {
  for (int i = 0; i < 3; ++i) {
    char *p = static_cast<char *>(__builtin_alloca(n + i));
    p[0] = 1;
    use(p);
  }
}
void f2(int n) { use(__builtin_alloca_with_align(n, 256)); }
void f3() { use(__builtin_alloca(16)); }
