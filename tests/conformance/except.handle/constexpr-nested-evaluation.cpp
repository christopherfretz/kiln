// [except.handle]/7, [except.throw] and [except.uncaught]/1 (C++26,
// P3068R6): each constant evaluation has its own exceptions. A constant
// expression evaluated while another evaluation is unwinding or inside a
// handler (here a static_assert in the final overrider of a virtual
// function, a member of a class template instantiated by the virtual call
// that the outer evaluation makes) sees no currently handled exception (a
// rethrow there has none to rethrow) and no uncaught exception, and its
// own throws are not "thrown while another is being unwound"; the outer
// evaluation's exception is intact afterwards.
//@ skip(clang): neither clang 19 nor clang 22 implements P3068
//@ expect: compile-ok
//@ strict-diags
namespace std {
constexpr int uncaught_exceptions() noexcept;
}

template <int N> constexpr int rethrow_none() {
  try {
    throw; // no exception is handled in this evaluation
  } catch (...) {
    return N;
  }
}

template <int N> constexpr int own_throw() {
  try {
    throw N; // not "thrown while another exception is being unwound"
  } catch (int e) {
    return e + std::uncaught_exceptions();
  }
}

struct ProbeBase {
  constexpr virtual int f() const { return 0; }
};

template <int N> struct Probe : ProbeBase {
  constexpr int f() const override {
    static_assert(!__builtin_constant_p(rethrow_none<N>()));
    static_assert(own_throw<N>() == N);
    return N;
  }
};

// The virtual call during unwinding instantiates Probe<1>::f: its
// static_asserts are evaluated then, nested in the outer evaluation.
struct Unwinder {
  int *out;
  constexpr ~Unwinder() {
    Probe<1> p;
    const ProbeBase &r = p;
    *out = r.f() * 10 + std::uncaught_exceptions();
  }
};

constexpr int during_unwinding() {
  int out = 0;
  try {
    Unwinder u{&out};
    throw 5;
  } catch (int e) {
    return out * 10 + e;
  }
}
// f() is 1, one uncaught exception in the destructor, then the handler.
static_assert(during_unwinding() == 115);

constexpr int in_handler() {
  try {
    try {
      throw 7;
    } catch (int) {
      Probe<2> p;
      const ProbeBase &r = p;
      if (r.f() != 2) return -1;
      throw; // still rethrows this evaluation's 7
    }
  } catch (int e) {
    return e;
  }
}
static_assert(in_handler() == 7);

int main() {}
