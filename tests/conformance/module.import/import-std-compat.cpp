// [std.modules]/3: `import std.compat;` also provides the C library names
// in the global namespace (::size_t, ::strlen, ::printf).
//@ expect: run
//@ std: c++20
//@ args: -stdlib=libc++ -Wno-reserved-module-identifier
//@ module: std=%{libcxx_modules}/std.cppm
//@ module: std.compat=%{libcxx_modules}/std.compat.cppm
//@ stdout: 5 5
import std.compat;

int main() {
  ::size_t n = ::strlen("hello");
  std::vector<char> v(n);
  ::printf("%zu %zu\n", n, v.size());
  return 0;
}
