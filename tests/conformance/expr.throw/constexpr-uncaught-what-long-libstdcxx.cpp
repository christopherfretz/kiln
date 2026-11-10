// [expr.const]/10 (C++26, P3068R6), [std.exceptions] (P3378R2,
// constexpr exception types): the note on a std::runtime_error built from
// a std::string, or a std::logic_error built from a string literal, that
// leaves the constant evaluation prints its what() whole at any length
// (WP-1245: past 1023 characters kiln printed the value dump of the
// object instead). libstdc++ 16.1's exception types are constexpr; the
// regexes pin the full text (spec/oven.md §Exceptions).
//@ verify
//@ args: --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): neither clang 19 nor clang 22 implements P3068
#include <stdexcept>
#include <string>

struct refusal : std::runtime_error {
  using std::runtime_error::runtime_error;
};
[[noreturn]] constexpr void raise(std::string msg) {
  throw refusal(msg); // #throw
}
// "widget: " then the digits 0-9 repeated to L characters.
constexpr int check(int len) {
  std::string m = "widget: ";
  while (m.size() < std::string::size_type(len))
    m += char('0' + (m.size() - 8) % 10);
  raise(m);
}
constexpr int a = check(1024); // expected-error {{constexpr variable 'a' must be initialized by a constant expression}}
// expected-note-re@#throw {{unhandled exception: widget: {{(0123456789){101}0}}12345{{$}}}}
constexpr int b = check(4000); // expected-error {{constexpr variable 'b' must be initialized by a constant expression}}
// expected-note-re@#throw {{unhandled exception: widget: {{((0123456789){133}){3}0}}1{{$}}}}

#define D10 "0123456789"
#define D100 D10 D10 D10 D10 D10 D10 D10 D10 D10 D10
#define D1000 D100 D100 D100 D100 D100 D100 D100 D100 D100 D100
constexpr int literal() {
  throw std::logic_error("LIT" D1000 D1000 "END"); // expected-note-re {{unhandled exception: LIT{{((0123456789){100}){2}E}}ND{{$}}}}
}
constexpr int c = literal(); // expected-error {{constexpr variable 'c' must be initialized by a constant expression}}
