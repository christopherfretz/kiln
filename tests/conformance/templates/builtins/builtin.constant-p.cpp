// [expr.const] (clang extension __builtin_constant_p): 1 if the argument
// (integral, enumeration, floating, pointer or nullptr_t) evaluates to a
// constant without side effects, 0 otherwise. A pointer constant is null or
// the start of a string literal. In a constant evaluation the argument is
// evaluated speculatively: a failure (an unordered pointer comparison, an
// inactive union member, an uninitialized read) is 0, not an error, as
// libc++'s __is_pointer_in_range relies on. At run time a non-constant
// argument is 0 and is not evaluated.
//@ expect: run
//@ strict-diags
//@ stdout: 2 0
constexpr int g = 3;
int h = 4;
static_assert(__builtin_constant_p(1));
static_assert(__builtin_constant_p(g + 1));
static_assert(!__builtin_constant_p(h));
static_assert(__builtin_constant_p(1.5));
static_assert(__builtin_constant_p(nullptr));
static_assert(__builtin_constant_p((int*)0));
static_assert(__builtin_constant_p("abc"));
static_assert(!__builtin_constant_p(&"abc"[1]));
static_assert(!__builtin_constant_p(&g));
struct S { int x; };
static_assert(!__builtin_constant_p(S{1}));
constexpr bool in_range(const int* b, const int* e, const int* p) {
  if (!__builtin_constant_p(b <= p && p < e))
    return false;
  return b <= p && p < e;
}
constexpr bool test() {
  int a[4] = {};
  int other = 0;
  bool ok = in_range(a, a + 4, a + 2) && !in_range(a, a + 4, &other);
  int n = 5;
  ok = ok && __builtin_constant_p(n) && !__builtin_constant_p(n++) && n == 5;
  int u;
  ok = ok && !__builtin_constant_p(u);
  ok = ok && __builtin_constant_p(a[1]) && !__builtin_constant_p(a[4]);
  return ok;
}
static_assert(test());
union U { int i; float f; };
constexpr bool active() {
  U u{.i = 1};
  return __builtin_constant_p(u.i) && !__builtin_constant_p(u.f);
}
static_assert(active());
static_assert(sizeof(__builtin_constant_p(0)) == sizeof(int));
template <class T> constexpr int cp(T t) { return __builtin_constant_p(t); }
static_assert(cp(1) == 1);
int rt(int x) { return __builtin_constant_p(x) + __builtin_constant_p(4) * 2; }
extern "C" int printf(const char*, ...);
int main(int argc, char**) { printf("%d %d\n", rt(argc), __builtin_constant_p(argc)); }
