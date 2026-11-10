// [expr.const]/10 (C++26, P3068R6), [lex.charset]: past the 64 KiB cap
// the note on an uncaught exception's what() cuts at a UTF-8 character
// boundary, never inside a multi-byte character, and "… [N more bytes]"
// counts every byte not printed, the split character's included
// (WP-1251: kiln cut at byte 65536 and printed U+FFFD for the part kept).
// The string is built in constant evaluation: P ASCII 'x' bytes, the
// UTF-8 character C, then "END". GCC 16.1 prints any length uncut
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

template <int P, int N> struct utf8_error : std::exception {
  char buf[P + N + 4];
  constexpr utf8_error(const char (&c)[N + 1]) : buf{} {
    for (int i = 0; i < P; ++i)
      buf[i] = 'x';
    for (int i = 0; i < N; ++i)
      buf[P + i] = c[i];
    buf[P + N] = 'E';
    buf[P + N + 1] = 'N';
    buf[P + N + 2] = 'D';
  }
  constexpr const char *what() const noexcept override { return buf; }
};

template <int P, int N> constexpr int raise(const char (&c)[N + 1]) {
  throw utf8_error<P, N>(c); // #throw
}

// U+00E9 (2 bytes) at bytes 65535-65536: 65535 printed, 2 + 3 counted.
constexpr int a = raise<65535, 2>("\xC3\xA9"); // expected-error {{constexpr variable 'a' must be initialized by a constant expression}}
// expected-note-re@#throw {{unhandled exception: {{x+}}… [5 more bytes]{{$}}}}
// U+20AC (3 bytes) at bytes 65534-65536: 65534 printed, 3 + 3 counted.
constexpr int b = raise<65534, 3>("\xE2\x82\xAC"); // expected-error {{constexpr variable 'b' must be initialized by a constant expression}}
// expected-note-re@#throw {{unhandled exception: {{x+}}… [6 more bytes]{{$}}}}
// U+1F600 (4 bytes) at bytes 65533-65536: 65533 printed, 4 + 3 counted.
constexpr int c = raise<65533, 4>("\xF0\x9F\x98\x80"); // expected-error {{constexpr variable 'c' must be initialized by a constant expression}}
// expected-note-re@#throw {{unhandled exception: {{x+}}… [7 more bytes]{{$}}}}
// U+00E9 ending at byte 65535, the last one printed: no cut, 3 counted.
constexpr int d = raise<65534, 2>("\xC3\xA9"); // expected-error {{constexpr variable 'd' must be initialized by a constant expression}}
// expected-note-re@#throw {{unhandled exception: {{x+}}é… [3 more bytes]{{$}}}}
