// Calls that do not return, and the C library memory functions: under
// -fsanitize=unreachable the call loses `noreturn` (call site and callee)
// and is followed by a builtin_unreachable check; memcpy/memmove/memset
// check their pointers against the declaration's `nonnull` (clang
// `EmitCall`, `EmitArgCheck`).
extern "C" {
void *memcpy(void *, const void *, unsigned long) __attribute__((nonnull(1, 2)));
void *memmove(void *, const void *, unsigned long) __attribute__((nonnull(1, 2)));
void *memset(void *, int, unsigned long) __attribute__((nonnull(1)));
}

[[noreturn]] void die();
[[noreturn]] void defined_after();
[[noreturn]] void defined_before() { for (;;) {} }
[[noreturn]] void never_called() { for (;;) {} }
typedef void (*Fatal)() __attribute__((noreturn));

struct S {
  [[noreturn]] void stop();
  [[noreturn]] static void st();
  [[noreturn]] virtual void v();
};
struct T { ~T(); };

int calls(int x, S *s, Fatal p) {
  if (x) die();
  if (x > 1) s->stop();
  if (x > 2) S::st();
  if (x > 3) s->v(); // through the vtable: returns
  if (x > 4) p();
  if (x > 5) defined_before();
  if (x > 6) defined_after();
  return x;
}

// The call becomes an `invoke`; the check follows in `invoke.cont`.
int with_cleanup(int x) {
  T t;
  if (x) die();
  return x;
}

[[noreturn]] void defined_after() { for (;;) {} }

void mem(void *d, const void *p, unsigned long n) {
  memcpy(d, p, n);
  memset(d, 0, n);
  __builtin_memcpy(d, p, n); // no `nonnull` attribute
  memmove(d, p, n);
}
