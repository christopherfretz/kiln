// [except.handle], [except.throw] (C++26, P3068R6) in a
// __builtin_constant_p trial (a GNU extension: 1 when its operand folds to
// a constant, else 0): a trial whose operand lets an exception escape is
// not a constant and yields 0 without ending the evaluation around it; an
// operand that throws and catches its own exception is a constant. A trial
// sees no exception of the evaluation it runs in (a rethrow there fails
// the trial), and that evaluation's handled or in-flight exception is
// untouched afterwards.
//@ skip(clang): neither clang 19 nor clang 22 implements P3068
//@ expect: compile-ok
//@ strict-diags
constexpr int thrower(int v) {
  if (v > 0) throw v;
  return v;
}

constexpr int catcher(int v) {
  try {
    return thrower(v);
  } catch (int e) {
    return -e;
  }
}

constexpr int rethrow() { throw; }

static_assert(!__builtin_constant_p(thrower(1)));
static_assert(__builtin_constant_p(thrower(0)));
static_assert(!__builtin_constant_p(catcher(3)));
static_assert(__builtin_constant_p(catcher(0)));
static_assert(!__builtin_constant_p(rethrow()));

constexpr int in_handler() {
  try {
    try {
      throw 6;
    } catch (int) {
      int trials = __builtin_constant_p(rethrow()) * 100 + __builtin_constant_p(catcher(0)) * 10 +
                   __builtin_constant_p(thrower(4));
      if (trials != 10) return trials;
      throw; // the evaluation's own exception is still the handled one
    }
  } catch (int e) {
    return e + 7;
  }
}
static_assert(in_handler() == 13);

struct Trial {
  int *out;
  constexpr ~Trial() { *out = __builtin_constant_p(thrower(1)) * 10 + __builtin_constant_p(catcher(0)); }
};

// A trial in a destructor run during unwinding: its own throw is not a
// throw while another exception is being unwound.
constexpr int during_unwinding() {
  int out = -1;
  try {
    Trial t{&out};
    throw 2;
  } catch (int e) {
    return out * 10 + e;
  }
}
static_assert(during_unwinding() == 12);

int main() {}
