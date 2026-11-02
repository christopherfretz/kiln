// [except.uncaught], [exception.syn] (C++26 after P3842R2, which withdrew
// the constexpr of std::uncaught_exceptions that P3068R6 had added):
// libc++ declares std::uncaught_exceptions() without constexpr, so a call
// to it is not a constant expression even in a function evaluated at
// compile time; the same function is fine at run time.
//@ expect: run
//@ args: -stdlib=libc++
//@ strict-diags
//@ stdout: 0 1
#include <exception>
#include <cstdio>

constexpr int count() { return std::uncaught_exceptions(); }

static_assert(!__builtin_constant_p(count()));

struct Observe {
  int *out;
  ~Observe() { *out = count(); }
};

int main() {
  int during = -1;
  try {
    Observe o{&during};
    throw 1;
  } catch (int) {
  }
  std::printf("%d %d\n", count(), during);
}
