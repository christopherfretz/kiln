// [dcl.enum]/5, [expr.call]/12: an enumeration whose underlying type is
// `bool` is passed and returned as a `bool` (`i1 zeroext` on x86-64 and
// AArch64, as clang lowers it), through direct calls, function pointers,
// virtual calls and by reference; a variadic argument of an unscoped one
// is promoted to `int`, a scoped one is not promoted and is read back with
// `va_arg` as itself. The shape is real code's
// `enum class Policy : bool` constructor parameter with a default.
//@ expect: run
//@ stdout: 1 0 1 1
//@ stdout: 3 1 1 0
//@ stdout: 1 1 0 1
extern "C" int printf(const char *, ...);
#include <stdarg.h>

enum class P : bool { NO, YES };
enum U : bool { UN, UY };

__attribute__((noinline)) P flip(P p) { return p == P::YES ? P::NO : P::YES; }
__attribute__((noinline)) U uflip(U u) { return u ? UN : UY; }
__attribute__((noinline)) P pick(int a, P x, char c, P y, long d) {
  return (a + c + d) % 2 ? x : y;
}
__attribute__((noinline)) void out(P &r, P v) { r = v; }
__attribute__((noinline)) P both(P a, U b) { return static_cast<P>(a == P::YES && b); }

struct Handler {
  explicit Handler(int n, P emit = P::YES) : n(n), emit(emit) {}
  virtual P get() const { return emit; }
  virtual ~Handler() = default;
  int n;
  const P emit;
};
struct Derived : Handler {
  using Handler::Handler;
  P get() const override { return flip(emit); }
};

__attribute__((noinline)) int sum(int n, ...) {
  va_list ap;
  va_start(ap, n);
  int s = 0;
  for (int i = 0; i < n; ++i) {
    s = s * 10 + va_arg(ap, int);
  }
  va_end(ap);
  return s;
}

__attribute__((noinline)) int scoped(int n, ...) {
  va_list ap;
  va_start(ap, n);
  int s = 0;
  for (int i = 0; i < n; ++i) {
    P p = va_arg(ap, P);
    s = s * 10 + (p == P::YES ? 1 : 0);
  }
  va_end(ap);
  return s;
}

int main() {
  P (*fp)(P) = flip;
  printf("%d %d %d %d\n", (int)flip(P::NO), (int)fp(P::YES), (int)uflip(UN),
         (int)pick(1, P::YES, 0, P::NO, 0));
  Handler h(1);
  Derived d(2, P::YES);
  const Handler &hr = d;
  P r = P::NO;
  out(r, h.get());
  printf("%d %d %d %d\n", h.n + d.n, (int)r, (int)h.get(), (int)hr.get());
  printf("%d %d %d %d\n", sum(2, UY, UN) == 10, scoped(3, P::YES, P::NO, P::YES) == 101,
         (int)both(P::YES, UN), (int)both(P::YES, UY));
}
