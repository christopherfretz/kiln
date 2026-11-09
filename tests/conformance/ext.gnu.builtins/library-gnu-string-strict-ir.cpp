// `stpcpy`, `stpncpy`, `strdup`, `strndup`, `index` and `rindex` are GNU
// library builtins only (clang's GNU_LANG builtins): under strict `c++20`
// a user declaration is an ordinary function, so the calls may throw and
// are `invoke`s under a pending cleanup. See library-gnu-string-nothrow-ir.cpp.
//@ expect: ir
//@ min-oracle: 22
//@ std: c++20
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
