// cold / hot function attributes (WP-172): on the declaration, the
// definition and every direct call site (clang `ConstructAttributeList`).
extern "C" [[noreturn]] void my_abort() noexcept __attribute__((cold));
__attribute__((cold)) void slow(int);
__attribute__((hot)) int fast(int x) { return x + 1; }
[[gnu::cold]] void slow_def(int x) { if (x) slow(x); }
struct S { __attribute__((cold)) void m(); };
void (*fp)(int) = slow;
int use(S& s, int x) {
  s.m();
  slow_def(x);
  fp(x);
  if (x > 3) my_abort();
  return fast(x);
}
