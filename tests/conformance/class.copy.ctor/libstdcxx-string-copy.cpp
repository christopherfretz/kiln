// [class.copy.ctor], [temp.explicit]/12, libstdc++ <string>: copying a
// std::string links and runs. libstdc++ declares `extern template class
// allocator<char>` and `extern template class basic_string<char>`; the copy
// constructor reaches `allocator<char>::allocate`/`deallocate`, which are
// `[[__gnu__::__always_inline__]]` and are not exported by libstdc++.so, so
// they must be emitted locally, and every exported member must be mangled
// with the Itanium std abbreviations (`SaIcE`, not `St9allocatorIcE`)
// ([abi] mangling, spec/abi.md). Verified against clang 19.
//@ expect: run
//@ strict-diags
//@ stdout: hello world, a long string beyond sso 37 5 hello
#include <cstdio>
#include <string>

int main() {
  std::string a = "hello world, a long string beyond sso";
  std::string b = a;            // copy constructor
  std::string c("hello");       // short string
  std::string d;
  d = c;                        // copy assignment
  std::string e(b.begin(), b.end());
  std::printf("%s %zu %zu %s\n", e.c_str(), b.size(), d.size(), d.c_str());
  return a == b && c == d ? 0 : 1;
}
