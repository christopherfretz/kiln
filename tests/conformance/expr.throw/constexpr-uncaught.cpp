// [expr.const]/10.? (C++26, P3068R6), [except.terminate]: an exception
// that leaves the constant evaluation, leaves a noexcept function, is
// rethrown with none handled, or is thrown while another is being unwound
// makes the expression non-constant (uncaught: clang's P3068 note wording,
// what(), else the value at the operand unless its type has size 1).
//@ skip(clang): neither clang 19 nor clang 22 implements P3068
//@ expect: compile-fail
//@ strict-diags
//@ diag: 47:15 error "constexpr variable 'a' must be initialized by a constant expression"
//@ diag: 46:38 note "unhandled exception of type 'int' with content 12 thrown from here"
//@ diag: 50:15 error "constexpr variable 'b' must be initialized by a constant expression"
//@ diag: 49:25 note "unhandled exception: boom"
//@ diag: 53:15 error "constexpr variable 'c' must be initialized by a constant expression"
//@ diag: 52:31 note "unhandled exception of type 'S' with content {4} thrown from here"
//@ diag: 56:15 error "constexpr variable 'd' must be initialized by a constant expression"
//@ diag: 55:45 note "uncaught exception in noexcept function"
//@ diag: 56:19 note "in call to 'unwrapped()'"
//@ diag: 59:15 error "constexpr variable 'e' must be initialized by a constant expression"
//@ diag: 58:32 note "there is no active exception to rethrow"
//@ diag: 59:19 note "in call to 'rethrow_none()'"
//@ diag: 72:15 error "constexpr variable 'f' must be initialized by a constant expression"
//@ diag: 62:47 note "exception thrown out of an object destructor while unrolling another exception"
//@ diag: 66:18 note "in call to 't.~ThrowsInDtor()'"
//@ diag: 72:19 note "in call to 'while_active()'"
//@ diag: 67:5 note "currently active exception thrown here"
//@ diag: 82:15 error "static assertion expression is not an integral constant expression"
//@ diag: 77:11 note "unhandled exception of type 'double' with content 1.500000e+00 thrown from here"
//@ diag: 93:15 error "constexpr variable 'g' must be initialized by a constant expression"
//@ diag: 92:37 note "unhandled exception of type 'Empty' thrown from here"
//@ diag: 95:15 error "constexpr variable 'h' must be initialized by a constant expression"
//@ diag: 94:34 note "unhandled exception of type 'OneChar' thrown from here"
//@ diag: 97:15 error "constexpr variable 'i' must be initialized by a constant expression"
//@ diag: 96:35 note "unhandled exception of type 'TwoChars' with content {120, 121} thrown from here"
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

// The content is omitted when the operand's type has size 1, as in
// clang's P3068 branch: an empty class (with a user-declared member
// function) and a class with one `char` member alike; two `char`s print.
struct Empty {
  constexpr int get() const { return 1; }
};
struct OneChar { char c; };
struct TwoChars { char c, d; };
constexpr int empty_class() { throw Empty(); }
constexpr int g = empty_class();
constexpr int one_char() { throw OneChar{'x'}; }
constexpr int h = one_char();
constexpr int two_chars() { throw TwoChars{'x', 'y'}; }
constexpr int i = two_chars();
