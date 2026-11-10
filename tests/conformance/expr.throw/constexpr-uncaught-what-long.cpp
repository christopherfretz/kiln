// [expr.const]/10 (C++26, P3068R6), [except.terminate]: the note on an
// exception that leaves the constant evaluation prints its what() whole,
// however long (WP-1245: kiln cut it at 1023 characters and fell back to
// the value dump). A user class with its own what() over a member buffer
// of 1023, 1024 and 4000 characters prints whole; past 64 KiB (70000
// characters) the note prints the first 65536 then "… [N more bytes]"
// (WP-1251: N counts bytes). A long string literal what() prints whole too. The
// regexes pin the full text (spec/oven.md §Exceptions).
//@ verify
//@ skip(clang): neither clang 19 nor clang 22 implements P3068
namespace std {
struct exception {
  constexpr exception() noexcept {}
  constexpr virtual ~exception() {}
  constexpr virtual const char *what() const noexcept { return "std::exception"; }
};
} // namespace std

// "BEGIN", then the digits 0-9 repeated, then "END": L characters.
template <int L> struct long_error : std::exception {
  char buf[L + 1];
  constexpr long_error() : buf{} {
    const char begin[] = "BEGIN";
    for (int i = 0; i < 5; ++i)
      buf[i] = begin[i];
    for (int i = 5; i < L - 3; ++i)
      buf[i] = char('0' + (i - 5) % 10);
    buf[L - 3] = 'E';
    buf[L - 2] = 'N';
    buf[L - 1] = 'D';
  }
  constexpr const char *what() const noexcept override { return buf; }
};

template <int L> constexpr int raise() {
  throw long_error<L>(); // #throw
}

constexpr int a = raise<1023>(); // expected-error {{constexpr variable 'a' must be initialized by a constant expression}}
// expected-note-re@#throw {{unhandled exception: BEGIN{{(0123456789){101}0}}1234END{{$}}}}
constexpr int b = raise<1024>(); // expected-error {{constexpr variable 'b' must be initialized by a constant expression}}
// expected-note-re@#throw {{unhandled exception: BEGIN{{(0123456789){101}0}}12345END{{$}}}}
constexpr int c = raise<4000>(); // expected-error {{constexpr variable 'c' must be initialized by a constant expression}}
// expected-note-re@#throw {{unhandled exception: BEGIN{{((0123456789){133}){3}0}}1END{{$}}}}
// 70000 = 65536 printed + 4464 counted; the printed part is "BEGIN" and
// 65531 digits, ending in '0'.
constexpr int d = raise<70000>(); // expected-error {{constexpr variable 'd' must be initialized by a constant expression}}
// expected-note-re@#throw {{unhandled exception: BEGIN{{(0123456789)+}}0… [4464 more bytes]{{$}}}}

// A string literal what() of 2006 characters.
#define D10 "0123456789"
#define D100 D10 D10 D10 D10 D10 D10 D10 D10 D10 D10
#define D1000 D100 D100 D100 D100 D100 D100 D100 D100 D100 D100
struct literal_error : std::exception {
  constexpr const char *what() const noexcept override { return "LIT" D1000 D1000 "END"; }
};
constexpr int literal() { throw literal_error(); } // expected-note-re {{unhandled exception: LIT{{((0123456789){100}){2}E}}ND{{$}}}}
constexpr int e = literal(); // expected-error {{constexpr variable 'e' must be initialized by a constant expression}}
