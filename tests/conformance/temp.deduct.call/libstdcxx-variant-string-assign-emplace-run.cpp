// [temp.deduct.call] through libstdc++'s <variant> (WP-381, WP-384):
// converting assignment, `emplace<0>` and a `std::string` alternative
// compile, link (the `std::__cxx11::basic_string` and `std::allocator<char>`
// symbols are spelled as libstdc++.so exports them) and run.
//@ expect: run
//@ args: -stdlib=libstdc++
//@ stdout: 1 x
#include <cstdio>
#include <string>
#include <variant>
int main() {
  std::variant<int, std::string> v;
  v = 1;
  v.emplace<0>(2);
  v = std::string("x");
  std::printf("%zu %s\n", v.index(), std::get<1>(v).c_str());
}
