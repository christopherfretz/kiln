// [temp.dep.expr]/3: a static data member of the current instantiation
// declared in its class as an array of unknown bound is type-dependent,
// so `sizeof(Ops)` in the template waits for instantiation. There the
// reference instantiates the member's out-of-line definition, whose
// initializer gives the bound ([temp.inst]/4, [dcl.array]/3; clang
// `completeExprArrayBound`), and `sizeof` sees the complete array, also
// for a member never odr-used (LLVM's demangler: `NumOps = sizeof(Ops) /
// sizeof(Ops[0])`).
//@ expect: run
//@ strict-diags
//@ std: c++17
//@ stdout: 3 3 4 2
#include <cstddef>
#include <cstdio>
struct Info { const char *name; int kind; };
template <class D> struct Parser {
  static const Info Ops[];
  static const std::size_t NumOps;
  static std::size_t inline_count() { return sizeof(Ops) / sizeof(Ops[0]); }
  static const int Small[];
};
template <class D>
const Info Parser<D>::Ops[] = {{"aa", 1}, {"ad", 2}, {"an", 3}};
template <class D>
const std::size_t Parser<D>::NumOps = sizeof(Ops) / sizeof(Ops[0]);
template <class D> const int Parser<D>::Small[] = {1, 2, 3, 4};
struct Tag {};
static_assert(sizeof(Parser<char>::Small) == 4 * sizeof(int), "");
static_assert(sizeof((Parser<long>::Ops)) == 3 * sizeof(Info), "");
int main() {
  std::printf("%zu %zu %zu %zu\n", Parser<Tag>::NumOps, Parser<int>::inline_count(),
              sizeof(Parser<Tag>::Small) / sizeof(int), (std::size_t)Parser<Tag>::Ops[1].kind);
}
