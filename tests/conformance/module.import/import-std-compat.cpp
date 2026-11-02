// [std.modules]/3: `import std.compat;` also provides the C library names
// in the global namespace (::size_t, ::strlen, ::printf).
//@ expect: run
//@ std: c++20
//@ args: -stdlib=libc++
//@ stdout: 5 5
//@ skip(clang): clang needs a prebuilt std.compat BMI (two-step --precompile of libc++'s std.compat.cppm) that the harness cannot drive
import std.compat;

int main() {
  ::size_t n = ::strlen("hello");
  std::vector<char> v(n);
  ::printf("%zu %zu\n", n, v.size());
  return 0;
}
