// [expr.const]/10.? (C++26, P3068R6), [except.terminate]: an exception
// that leaves the constant evaluation, leaves a noexcept function, is
// rethrown with none handled, or is thrown while another is being unwound
// makes the expression non-constant (the uncaught exception note is worded
// as clang's P3068 implementation: what(), else the value at the operand).
//@ skip(clang): neither clang 19 nor clang 22 implements P3068
//@ expect: compile-fail
//@ strict-diags
//@ diag: 41:15 error "constexpr variable 'a' must be initialized by a constant expression"
//@ diag: 40:38 note "unhandled exception of type 'int' with content 12 thrown from here"
//@ diag: 44:15 error "constexpr variable 'b' must be initialized by a constant expression"
//@ diag: 43:25 note "unhandled exception: boom"
//@ diag: 47:15 error "constexpr variable 'c' must be initialized by a constant expression"
//@ diag: 46:31 note "unhandled exception of type 'S' with content {4} thrown from here"
//@ diag: 50:15 error "constexpr variable 'd' must be initialized by a constant expression"
//@ diag: 49:45 note "uncaught exception in noexcept function"
//@ diag: 50:19 note "in call to 'unwrapped()'"
//@ diag: 53:15 error "constexpr variable 'e' must be initialized by a constant expression"
//@ diag: 52:32 note "rethrow with no active exception"
//@ diag: 53:19 note "in call to 'rethrow_none()'"
//@ diag: 66:15 error "constexpr variable 'f' must be initialized by a constant expression"
//@ diag: 56:47 note "throw while another exception is already active"
//@ diag: 60:18 note "in call to 't.~ThrowsInDtor()'"
//@ diag: 66:19 note "in call to 'while_active()'"
//@ diag: 61:5 note "currently active exception thrown here"
//@ diag: 76:15 error "static assertion expression is not an integral constant expression"
//@ diag: 71:11 note "unhandled exception of type 'double' with content 1.500000e+00 thrown from here"
namespace std {
struct exception {
  constexpr exception() noexcept {}
  constexpr virtual ~exception() {}
  constexpr virtual const char *what() const noexcept { return "std::exception"; }
};
} // namespace std
struct my_error : std::exception {
  constexpr const char *what() const noexcept override { return "boom"; }
};
struct S { int v; };

constexpr int thrower(int v) { throw v; }
constexpr int a = thrower(12);

constexpr int what_() { throw my_error(); }
constexpr int b = what_();

constexpr int value() { throw S{4}; }
constexpr int c = value();

constexpr int unwrapped() noexcept { return thrower(1); }
constexpr int d = unwrapped();

constexpr int rethrow_none() { throw; }
constexpr int e = rethrow_none();

struct ThrowsInDtor {
  constexpr ~ThrowsInDtor() noexcept(false) { throw 2; }
};
constexpr int while_active() {
  try {
    ThrowsInDtor t;
    throw 1;
  } catch (...) {
  }
  return 0;
}
constexpr int f = while_active();

// Not caught by a handler of an unrelated type.
constexpr int mismatch() {
  try {
    throw 1.5;
  } catch (int) {
  }
  return 0;
}
static_assert(mismatch() == 0);
