// [expr.const]/10 (C++26, P3068R6), [expr.const]/10.8, [expr.add]/4: a
// what() string whose scan faults after the first character (no null
// character within its array, or an uninitialized character) prints the
// characters read, then why the scan stopped, in the words of the note
// the failing read carries (as a static_assert message prints what it
// read; WP-1251: kiln printed the value dump instead). Nothing read
// keeps the value dump. GCC 16.1 prints the value dump for all of these
// (spec/oven.md §Exceptions).
//@ verify
//@ skip(clang): neither clang 19 nor clang 22 implements P3068
namespace std {
struct exception {
  constexpr exception() noexcept {}
  constexpr virtual ~exception() {}
  constexpr virtual const char *what() const noexcept { return "std::exception"; }
};
} // namespace std

// No null character within `buf`: the read of buf[4] is past its end
// (the member after it is not read).
struct unterminated : std::exception {
  char buf[4] = {'a', 'b', 'c', 'd'};
  int tail = 0;
  constexpr const char *what() const noexcept override { return buf; }
};
constexpr int a() { throw unterminated(); } // expected-note {{unhandled exception: abcd… [end of what() string not reached: read of dereferenced one-past-the-end pointer is not allowed in a constant expression]}}
constexpr int ra = a(); // expected-error {{constexpr variable 'ra' must be initialized by a constant expression}}

// The third character is uninitialized.
struct partial : std::exception {
  char buf[8];
  constexpr partial() {
    buf[0] = 'h';
    buf[1] = 'i';
  }
  constexpr const char *what() const noexcept override { return buf; }
};
constexpr int b() { throw partial(); } // expected-note {{unhandled exception: hi… [end of what() string not reached: read of uninitialized object is not allowed in a constant expression]}}
constexpr int rb = b(); // expected-error {{constexpr variable 'rb' must be initialized by a constant expression}}

// The first character is uninitialized: nothing read, the value dump.
struct unread : std::exception {
  char buf[2];
  constexpr unread() {}
  constexpr const char *what() const noexcept override { return buf; }
};
constexpr int c() { throw unread(); } // expected-note-re {{unhandled exception of type 'unread' with content {{[{]{2}[}], [{]}}<uninitialized>, <uninitialized>{{[}][}]}} thrown from here}}
constexpr int rc = c(); // expected-error {{constexpr variable 'rc' must be initialized by a constant expression}}

// Unterminated past the 64 KiB cap: the first 65536 bytes, the rest
// counted, then the reason (70000 = 65536 + 4464).
template <int L> struct long_unterminated : std::exception {
  char buf[L];
  constexpr long_unterminated() {
    for (int i = 0; i < L; ++i)
      buf[i] = 'x';
  }
  constexpr const char *what() const noexcept override { return buf; }
};
constexpr int d() { throw long_unterminated<70000>(); } // expected-note-re {{unhandled exception: {{x+}}… [4464 more bytes; end of what() string not reached: read of dereferenced one-past-the-end pointer is not allowed in a constant expression]{{$}}}}
constexpr int rd = d(); // expected-error {{constexpr variable 'rd' must be initialized by a constant expression}}
