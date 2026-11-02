// [except.uncaught]/1 (C++26, P3068R6): an exception is uncaught from the
// completed initialization of the exception object until a handler for it
// is activated (or std::terminate). When std::uncaught_exceptions is
// declared constexpr (P3068R6 did so; P3842R2 withdrew it, and libc++
// keeps it non-constexpr: see uncaught-exceptions-libcxx-not-constexpr.cpp)
// kiln evaluates it as that count in a constant evaluation.
//@ skip(clang): neither clang 19 nor clang 22 implements P3068
//@ expect: compile-ok
//@ strict-diags
namespace std {
constexpr int uncaught_exceptions() noexcept;
}

struct Inner {
  int *out;
  constexpr ~Inner() { *out = *out * 10 + std::uncaught_exceptions(); }
};

// Destroyed while the outer exception unwinds: one uncaught, then two
// while its own exception unwinds Inner, one again in its handler.
struct Outer {
  int *out;
  constexpr ~Outer() {
    *out = *out * 10 + std::uncaught_exceptions();
    try {
      Inner i{out};
      throw 2;
    } catch (int) {
      *out = *out * 10 + std::uncaught_exceptions();
    }
  }
};

constexpr int counts() {
  int out = 0;
  out = out * 10 + std::uncaught_exceptions();
  try {
    Outer o{&out};
    throw 1;
  } catch (int) {
    out = out * 10 + std::uncaught_exceptions();
    try {
      Inner i{&out};
      throw; // a rethrown exception is uncaught again until caught
    } catch (int) {
    }
  }
  return out * 10 + std::uncaught_exceptions();
}
// 0 before, 1 in ~Outer, 2 in ~Inner, 1 in the inner handler, 0 in the
// outer handler, 1 in ~Inner during the rethrow, 0 after.
static_assert(counts() == 121010);

int main() {}
