// [std.modules], [format], [print]: `import std;` under C++23 provides
// std::format and std::println.
//@ expect: run
//@ std: c++23
//@ args: -stdlib=libc++ -Wno-reserved-module-identifier
//@ module: std=%{libcxx_modules}/std.cppm
//@ stdout: x=42 y=ok
//@ stdout: 3.50|  7|
import std;

int main() {
  std::string s = std::format("x={} y={}", 42, "ok");
  std::println("{}", s);
  std::println("{:.2f}|{:>3}|", 3.5, 7);
  return s.size() == 9 ? 0 : 1;
}
