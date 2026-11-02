// [temp.explicit]/12 and the Itanium ABI tags of libstdc++'s
// `std::__cxx11` (WP-384): `std::locale::name()` returns a tagged
// `std::__cxx11::string`, so it is `_ZNKSt6locale4nameB5cxx11Ev`; the
// `std::string` members come from libstdc++.so, except the
// `always_inline` `std::allocator<char>` members, defined here.
//@ expect: run
//@ args: -stdlib=libstdc++
//@ stdout: C
#include <cstdio>
#include <locale>
#include <string>
int main() {
  std::string n = std::locale().name();
  std::printf("%s\n", n.c_str());
}
