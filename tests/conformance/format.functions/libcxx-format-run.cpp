// [format.functions]/2, /5, /11: std::format and std::format_to with
// libc++ 19 compile and run: std::format -> std::vformat ->
// std::vformat_to -> std::__vformat_to deduces `_CharT` through the
// defaulted `basic_string_view<_CharT>` ([temp.deduct.type]/9; WP-286),
// the format string is checked by the consteval basic_format_string
// constructor ([format.fmt.string]/2), and the parser's bit-field default
// member initializers are stored ([class.base.init]/9). Matches clang 19.
//@ args: -stdlib=libc++
//@ expect: run
//@ stdout: 1|a 2.5|[  42|ff]|x-7
#include <cstdio>
#include <format>
#include <iterator>
#include <string>

int main() {
  std::string a = std::format("{}", 1);
  std::string b = std::format("{} {}", "a", 2.5);
  std::string c;
  std::format_to(std::back_inserter(c), "[{:>4}|{:x}]", 42, 255u);
  std::string d;
  std::format_to(std::back_inserter(d), "{}{}", 'x', -7);
  std::printf("%s|%s|%s|%s\n", a.c_str(), b.c_str(), c.c_str(), d.c_str());
}
