// Clang extension: padded _Atomic objects of class type at run time:
// initialization from the value type, assignment from T and _Atomic(T),
// and the __c11_atomic builtins on them (the value is copied through an
// atomic-width temporary; the padding stays zero). The cmpxchg is one
// that must fail: a successful compare of a padded value depends on the
// expected temporary's padding bytes, which clang leaves uninitialized
// (it succeeds by luck on aarch64 and not on x86-64; WP-228). clang 19
// writes the failed compare's old value only into that temporary, not
// back into `e`, so `e` keeps its value.
//@ expect: run
//@ stdout: abc abc xyz
//@ stdout: xyz 0 abc abc
//@ stdout: 4 0
extern "C" int printf(const char *, ...);
struct S3 { char c[3]; };
_Atomic(S3) g;
void show(S3 s) { printf("%c%c%c", s.c[0], s.c[1], s.c[2]); }
int main() {
  S3 a{{'a', 'b', 'c'}};
  _Atomic(S3) x = a;
  _Atomic(S3) y{a};
  g = x;
  show(__c11_atomic_load(&g, 5)); printf(" ");
  show(__c11_atomic_load(&y, 5)); printf(" ");
  S3 b{{'x', 'y', 'z'}};
  y = b;
  show(__c11_atomic_load(&y, 5)); printf("\n");
  S3 old = __c11_atomic_exchange(&g, b, 5);
  show(__c11_atomic_load(&g, 5));
  S3 q{{'q', 'r', 's'}};
  S3 e = a;
  bool ok = __c11_atomic_compare_exchange_strong(&g, &e, q, 5, 5);
  printf(" %d ", ok); show(e); printf(" ");
  __c11_atomic_store(&g, a, 5);
  show(__c11_atomic_load(&g, 5)); printf("\n");
  (void)old;
  unsigned char raw[4];
  __builtin_memcpy(raw, &x, 4);
  printf("%d %d\n", (int)sizeof(x), raw[3]);
}
