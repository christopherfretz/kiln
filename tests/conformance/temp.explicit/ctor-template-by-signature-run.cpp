// [temp.explicit]/4, /11, [temp.deduct.decl], [temp.param]/12: the
// constructor-template shape of a string-like class header (an
// `enable_if`-constrained forwarding constructor, its defaulted non-type
// template parameter not repeated by the out-of-line definition, and an
// `extern template` naming the specialization by signature at the end of
// the header). The explicit instantiation declaration deduces `T =
// std::string` and the defaulted parameter from its default; the
// definition that follows (as the library's source file would provide)
// emits the specialization, so the program links. WP-580.
//@ expect: run
//@ strict-diags
//@ stdout: 2 3
#include <cstdio>
#include <string>
#include <type_traits>
struct Cord {
  template <typename T, std::enable_if_t<std::is_same<T, std::string>::value, int> = 0>
  explicit Cord(T &&src);
  int n = 0;
};
template <typename T, std::enable_if_t<std::is_same<T, std::string>::value, int>>
Cord::Cord(T &&src) : n((int)src.size()) {}
extern template Cord::Cord(std::string &&src);

int main() {
  Cord c(std::string("ab"));
  std::string s = "abc";
  Cord d(std::move(s));
  std::printf("%d %d\n", c.n, d.n);
  return c.n - 2;
}

template Cord::Cord(std::string &&src);
