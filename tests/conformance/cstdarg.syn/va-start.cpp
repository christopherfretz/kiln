// [cstdarg.syn]/1, C 7.16.1: `__builtin_va_start` / `va_copy` / `va_end`
// lower to the `llvm.va_*` intrinsics on the `va_list` object, which is
// then handed to a `v*printf` function (by value: the AAPCS64 `va_list`
// is a struct passed indirectly as a copy). The second `va_start`
// operand is not evaluated.
//@ expect: run
//@ stdout: 1-2-x
//@ stdout: 7 8 9|7 8 9
//@ stdout: m 3.5
//@ stdout: t 42
extern "C" int printf(const char *, ...);
extern "C" int vsnprintf(char *, unsigned long, const char *, __builtin_va_list);
extern "C" int vprintf(const char *, __builtin_va_list);

int format(char *buf, const char *f, ...) {
  __builtin_va_list ap;
  __builtin_va_start(ap, f);
  int r = vsnprintf(buf, 64, f, ap);
  __builtin_va_end(ap);
  return r;
}

void twice(const char *f, ...) {
  __builtin_va_list ap, aq;
  __builtin_va_start(ap, f);
  __builtin_va_copy(aq, ap);
  vprintf(f, ap);
  printf("|");
  vprintf(f, aq);
  printf("\n");
  __builtin_va_end(aq);
  __builtin_va_end(ap);
}

struct S {
  void say(const char *f, ...) {
    __builtin_va_list ap;
    __builtin_va_start(ap, f);
    vprintf(f, ap);
    __builtin_va_end(ap);
  }
};

template <class T> void say(T f, ...) {
  __builtin_va_list ap;
  __builtin_va_start(ap, f);
  vprintf(f, ap);
  __builtin_va_end(ap);
}

int main() {
  char buf[64];
  int n = format(buf, "%d-%d-%c", 1, 2, 'x');
  printf("%s\n", buf);
  twice("%d %d %d", 7, 8, 9);
  S{}.say("m %.1f\n", 3.5);
  say<const char *>("t %d\n", 42);
  return n == 5 ? 0 : 1;
}
