//@ args: -O2
// `disable_tail_calls` / `clang::disable_tail_calls` (clang
// `DisableTailCallsAttr`, clang `ConstructAttributeList`, WP-682): a
// function declaration or definition carries
// `"disable-tail-calls"="true"`, inherited from an earlier declaration
// too; call sites do not, and a member function takes
// it like any other. googletest's `GTEST_NO_TAIL_CALL_` spells it.
int g(int);
__attribute__((disable_tail_calls)) int f(int x);
int f(int x) { return g(x); }
[[clang::disable_tail_calls]] int h(int x) { return g(x + 1); }
__attribute__((disable_tail_calls)) int d(int x);
struct S {
  [[clang::disable_tail_calls]] int m(int x) { return g(x) + 1; }
};
template <class T> __attribute__((disable_tail_calls)) T t(T x) { return g(x); }
int plain(int x) { return g(x); }
int use(S& s, int x) { return s.m(x) + d(x) + t(x) + plain(x); }
