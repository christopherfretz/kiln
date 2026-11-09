// clang's `__builtin_`-prefixed aliases of C library functions that need
// no declaration (Builtins.td `LibBuiltin` with `AddBuiltinPrefixedAlias`
// and the plain `__builtin_` rows; WP-1104): each lowers to a call of the
// library function, `nounwind` for the NoThrow ones (malloc, calloc,
// realloc, free, fprintf, vfprintf, the string functions except
// strcasecmp/strncasecmp), with no `allocsize`. The `FILE` ones exist only
// once a `FILE` type is declared. `__builtin_alloca_uninitialized` and
// `__builtin_alloca_with_align_uninitialized` are dynamic `alloca`s.
typedef struct _IO_FILE FILE;
void sink(void *, void *);

void *alloc(__SIZE_TYPE__ n) {
  void *p = __builtin_malloc(n);
  void *q = __builtin_calloc(n, 4);
  p = __builtin_realloc(p, 2 * n);
  __builtin_free(q);
  return p;
}

__SIZE_TYPE__ str(char *a, const char *p) {
  __builtin_strcpy(a, p);
  __builtin_strncpy(a, p, 3);
  __builtin_strcat(a, p);
  __builtin_strncat(a, p, 3);
  __builtin_stpcpy(a, p);
  __builtin_stpncpy(a, p, 2);
  __builtin_free(__builtin_strdup(p));
  __builtin_free(__builtin_strndup(p, 2));
  __builtin_index(p, 'x');
  __builtin_rindex(p, 'y');
  int c = __builtin_strcasecmp(a, p) + __builtin_strncasecmp(a, p, 2);
  return __builtin_strspn(a, p) + __builtin_strcspn(a, p) + c;
}

int io(FILE *in, FILE *out, const char *fmt, __builtin_va_list ap) {
  int x = 0;
  __builtin_fscanf(in, "%d", &x);
  __builtin_vfscanf(in, fmt, ap);
  __builtin_vfprintf(out, fmt, ap);
  return __builtin_fprintf(out, "%d\n", x);
}

void stack(__SIZE_TYPE__ n) {
  sink(__builtin_alloca_uninitialized(n),
       __builtin_alloca_with_align_uninitialized(n, 64));
}
