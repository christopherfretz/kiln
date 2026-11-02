// GNU __null (spec/sema.md §5.8): an integer null pointer constant of the
// signed integer type as wide as a pointer (long on LP64); it converts to
// any pointer, pointer to member and std::nullptr_t, prefers an exact
// match on long in overload resolution, and is usable in constant
// expressions.
//@ expect: run
//@ stdout: 0 1 1 1 1 1 3 0
extern "C" int printf(const char*, ...);
#define NULL __null
struct S { int m; };
long getl() { return __null; }
int *getp() { return NULL; }
int over(long) { return 1; }
int over(...) { return 2; }
int pick(void *) { return 3; }
constexpr int *cp = NULL;
constexpr long cl = __null;
static_assert(__is_same(decltype(__null), long));
static_assert(sizeof(__null) == sizeof(void *));
static_assert(cp == nullptr && cl == 0L && !(long)__null);
int main() {
  int S::*mp = NULL;
  void (*fp)() = __null;
  int *p = __null;
  printf("%ld %d %d %d %d %d %d %ld\n", getl(), getp() == nullptr, mp == nullptr, fp == nullptr,
         p == NULL, over(__null), pick(NULL), cl);
  return cp != nullptr;
}
