// clang's `EmitBuiltinExpr` first tries to evaluate a builtin call as an
// rvalue and emits the constant when that succeeds, also for a library
// builtin such as `strlen` (not a constant expression, but the evaluator
// folds it, clang `VisitBuiltinCallExpr`'s `BIstrlen` cases), without
// emitting the arguments; a global's initializer folds through the
// constant emitter. A pointer result (`strchr`) is not kept in a function,
// a `no_builtin` function keeps the call, and a non-constant argument
// keeps it too. WP-940: kiln emitted the library call. Found by the WP-871
// fmt smoke: gtest's `strlen(prefix)` on a static const array gets a
// nonnull-attribute check under kiln but is the constant 7 under clang.
//@ expect: ir
//@ min-oracle: 22
//@ std: c++20
extern "C" {
__SIZE_TYPE__ strlen(const char *);
__SIZE_TYPE__ wcslen(const wchar_t *);
int strcmp(const char *, const char *);
int strncmp(const char *, const char *, __SIZE_TYPE__);
int wcscmp(const wchar_t *, const wchar_t *);
int memcmp(const void *, const void *, __SIZE_TYPE__);
int wmemcmp(const wchar_t *, const wchar_t *, __SIZE_TYPE__);
char *strchr(const char *, int);
}

__SIZE_TYPE__ local_array() {
  static const char prefix[] = "std::__";
  return strlen(prefix);
}

__SIZE_TYPE__ literal() { return strlen("abc"); }

static const char hello[] = "hello";
__SIZE_TYPE__ global_len = strlen("abcd") + 1;
const char *global_chr = strchr(hello, 'l');
int global_cmp = strcmp("a", "b");
__SIZE_TYPE__ wide_len() { return wcslen(L"abcd"); }
int bounded_cmp() { return strncmp("abc", "abd", 2); }
int wide_cmp() { return wcscmp(L"b", L"a"); }
int mem_cmp() { return memcmp(hello, "help", 4) < 0; }
int wide_mem_cmp() { return wmemcmp(L"ab", L"ab", 2); }
char *pointer_result() { return strchr(hello, 'l'); }
__attribute__((no_builtin("strlen"))) __SIZE_TYPE__ no_builtin() { return strlen("abc"); }
__SIZE_TYPE__ not_constant(int i) { return strlen(i ? "a" : "bb"); }
int constant_p() { return __builtin_constant_p(strlen("ab")); }
