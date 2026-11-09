// The string `__builtin_` aliases of the C library (Builtins.td
// `LibBuiltin`/`GNULibBuiltin` with `AddBuiltinPrefixedAlias`, WP-1104):
// `__builtin_strcpy`, `strncpy`, `strcat`, `strncat`, `strspn`,
// `strcspn`, `strdup`, `strndup`, `stpcpy`, `stpncpy`, `index`, `rindex`
// (NoThrow) and `strcasecmp`, `strncasecmp` (not NoThrow). They are
// declared in every dialect, need no declaration, and lower to calls of
// the library functions; the NoThrow ones are `nounwind` calls under a
// pending cleanup, the others `invoke`s (GNU [dcl.attr] extension).
// Pinned to oracle 22 like every ir test here (clang 19 and 22 differ in
// target attributes); tests/codegen/lib_builtin_fwd.cpp covers oracle 19.
//@ expect: ir
//@ min-oracle: 22
//@ std: c++20
struct D { ~D(); };

__SIZE_TYPE__ f(char *a, const char *p) {
  D d;
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
