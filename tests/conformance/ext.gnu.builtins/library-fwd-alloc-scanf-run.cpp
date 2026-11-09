// The `__builtin_` library aliases at run time (WP-1104; GNU [dcl.attr]
// extension): memory from `__builtin_malloc`/`__builtin_calloc` grows
// with `__builtin_realloc` and is released with `__builtin_free`, and
// `__builtin_fscanf` / `__builtin_vfscanf` read from an `fmemopen`
// stream, `__builtin_fprintf` / `__builtin_vfprintf` write to stdout.
//@ expect: run
//@ std: c++20
//@ stdout: 7 11 13 hello 0 0 24 | 5 x
#include <cstdarg>
#include <cstdio>

static int vread(FILE *in, const char *fmt, ...) {
  va_list ap;
  va_start(ap, fmt);
  int n = __builtin_vfscanf(in, fmt, ap);
  va_end(ap);
  return n;
}

static void vwrite(const char *fmt, ...) {
  va_list ap;
  va_start(ap, fmt);
  __builtin_vfprintf(stdout, fmt, ap);
  va_end(ap);
}

int main() {
  char text[] = "7 11 13 hello 5 x";
  FILE *in = fmemopen(text, sizeof text - 1, "r");
  if (!in)
    return 1;
  int *v = static_cast<int *>(__builtin_malloc(2 * sizeof(int)));
  if (__builtin_fscanf(in, "%d %d", &v[0], &v[1]) != 2)
    return 2;
  v = static_cast<int *>(__builtin_realloc(v, 3 * sizeof(int)));
  if (__builtin_fscanf(in, "%d", &v[2]) != 1)
    return 3;
  char *word = static_cast<char *>(__builtin_calloc(16, 1));
  if (__builtin_fscanf(in, "%15s", word) != 1)
    return 4;
  char *copy = __builtin_strdup(word);
  int *z = static_cast<int *>(__builtin_calloc(2, sizeof(int)));
  __builtin_fprintf(stdout, "%d %d %d %s %d %d %d", v[0], v[1], v[2], copy,
                    z[0], z[1], v[0] + v[1] + v[2] - 7);
  int k = 0;
  char c = 0;
  if (vread(in, " %d %c", &k, &c) != 2)
    return 5;
  vwrite(" | %d %c\n", k, c);
  __builtin_free(z);
  __builtin_free(copy);
  __builtin_free(word);
  __builtin_free(v);
  fclose(in);
}
