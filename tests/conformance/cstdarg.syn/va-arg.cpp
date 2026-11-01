// [cstdarg.syn]/1, C 7.16.1.1: `__builtin_va_arg` reads the next variadic
// argument of the given type (lowered as clang's target `EmitVAArg`: the
// AAPCS64 / SysV x86-64 register-save-area walk with its overflow-area
// fallback). Covers integers, pointers, `double`, `long double`, small,
// large and over-aligned structs, a homogeneous float aggregate, a mixed
// int/float struct, a read through a `va_list` parameter, enough reads to
// spill past the register save area, and a discarded read.
//@ expect: run
//@ stdout: 1 -2 300000000000 x
//@ stdout: 0.5 2.25 -7.125
//@ stdout: s 3 4 | l 10 11 12 13 14 | a 99
//@ stdout: f 1.5 2.5 3.5 | m 7 8.5
//@ stdout: sum 120 | skip 9 | ld 3.25
extern "C" int printf(const char *, ...);

struct S { int a, b; };
struct L { long v[5]; };
struct alignas(16) A16 { long x; };
struct F3 { float x, y, z; };
struct M { int i; double d; };

void ints(int n, ...) {
  __builtin_va_list ap;
  __builtin_va_start(ap, n);
  int a = __builtin_va_arg(ap, int);
  int b = __builtin_va_arg(ap, int);
  long c = __builtin_va_arg(ap, long);
  const char *p = __builtin_va_arg(ap, const char *);
  printf("%d %d %ld %s\n", a, b, c, p);
  __builtin_va_end(ap);
}

void dbls(int n, ...) {
  __builtin_va_list ap;
  __builtin_va_start(ap, n);
  for (int i = 0; i < n; ++i)
    printf(i ? " %g" : "%g", __builtin_va_arg(ap, double));
  printf("\n");
  __builtin_va_end(ap);
}

void aggs(int n, ...) {
  __builtin_va_list ap;
  __builtin_va_start(ap, n);
  S s = __builtin_va_arg(ap, S);
  L l = __builtin_va_arg(ap, L);
  A16 a = __builtin_va_arg(ap, A16);
  printf("s %d %d | l %ld %ld %ld %ld %ld | a %ld\n", s.a, s.b, l.v[0], l.v[1],
         l.v[2], l.v[3], l.v[4], a.x);
  F3 f = __builtin_va_arg(ap, F3);
  int mi = __builtin_va_arg(ap, M).i;
  M m = __builtin_va_arg(ap, M);
  printf("f %g %g %g | m %d %g\n", f.x, f.y, f.z, mi + m.i - 7, m.d);
  __builtin_va_end(ap);
}

long sumv(int n, __builtin_va_list ap) {
  long s = 0;
  for (int i = 0; i < n; ++i)
    s += __builtin_va_arg(ap, int);
  return s;
}

long sum(int n, ...) {
  __builtin_va_list ap;
  __builtin_va_start(ap, n);
  long s = sumv(n, ap);
  __builtin_va_end(ap);
  return s;
}

void skip(int n, ...) {
  __builtin_va_list ap;
  __builtin_va_start(ap, n);
  (void)__builtin_va_arg(ap, int);
  (void)__builtin_va_arg(ap, S);
  int v = __builtin_va_arg(ap, int);
  long double ld = __builtin_va_arg(ap, long double);
  printf("skip %d | ld %.2Lf\n", v, ld);
  __builtin_va_end(ap);
}

int main() {
  ints(0, 1, -2, 300000000000L, "x");
  dbls(3, 0.5, 2.25, -7.125);
  aggs(0, S{3, 4}, L{{10, 11, 12, 13, 14}}, A16{99}, F3{1.5f, 2.5f, 3.5f},
       M{7, 0.0}, M{7, 8.5});
  printf("sum %ld | ", sum(15, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15));
  skip(0, 1, S{5, 6}, 9, 3.25L);
  return 0;
}
