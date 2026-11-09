// `stpcpy`, `stpncpy`, `strdup`, `strndup`, `index` and `rindex` are
// `nothrow` library builtins in GNU dialects only (clang's GNU_LANG
// builtins): under `gnu++20` a user declaration gets `nounwind` and the
// calls are `call`s under a pending cleanup (clang's
// `AddKnownFunctionAttributes`). See library-gnu-string-strict-ir.cpp.
//@ expect: ir
//@ min-oracle: 22
//@ args: -std=gnu++20
extern "C" char *stpcpy(char *, const char *);
extern "C" char *stpncpy(char *, const char *, __SIZE_TYPE__);
extern "C" char *strdup(const char *);
extern "C" char *strndup(const char *, __SIZE_TYPE__);
extern "C" char *index(const char *, int);
extern "C" char *rindex(const char *, int);

struct D { ~D(); };

char *f(char *a, const char *p) {
  D d;
  stpcpy(a, p);
  stpncpy(a, p, 2);
  strdup(p);
  strndup(p, 2);
  index(p, 'x');
  return rindex(p, 'y');
}
