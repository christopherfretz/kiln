// [expr.const]/10 (C++26, P3068R6), [except.terminate]: when the what()
// of an uncaught exception is itself not a constant expression (it
// throws out of a noexcept function, or divides by zero), the note keeps
// the value dump of the exception object, and the inner failure is not
// reported (WP-1251). GCC 16.1 prints the value dump too, after an
// error for the inner failure (spec/oven.md §Exceptions).
//@ verify
//@ skip(clang): neither clang 19 nor clang 22 implements P3068
namespace std {
struct exception {
  constexpr exception() noexcept {}
  constexpr virtual ~exception() {}
  constexpr virtual const char *what() const noexcept { return "std::exception"; }
};
} // namespace std

// Throws out of the noexcept what().
struct throws : std::exception {
  int code = 7;
  static constexpr const char *boom() { throw 1; }
  constexpr const char *what() const noexcept override { return boom(); }
};
constexpr int a() { throw throws(); } // expected-note-re {{unhandled exception of type 'throws' with content {{[{]{2}[}], 7[}]}} thrown from here}}
constexpr int ra = a(); // expected-error {{constexpr variable 'ra' must be initialized by a constant expression}}

// Divides by zero.
struct faults : std::exception {
  int d = 0;
  constexpr const char *what() const noexcept override { return "x" + 10 / d; }
};
constexpr int b() { throw faults(); } // expected-note-re {{unhandled exception of type 'faults' with content {{[{]{2}[}], 0[}]}} thrown from here}}
constexpr int rb = b(); // expected-error {{constexpr variable 'rb' must be initialized by a constant expression}}
