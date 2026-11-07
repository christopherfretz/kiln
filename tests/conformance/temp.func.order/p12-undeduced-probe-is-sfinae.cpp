//@ expect: run
//@ stdout: 1 2 1 2
// WP-782: [temp.deduct.partial]/12, [temp.deduct.general]/8: checking
// whether an undeduced template parameter is named by the compared
// parameter types is a substitution failure, never a diagnostic. With
// `P` explicit, `get<Opt>(p)` has two viable candidates; partial ordering
// leaves `P` undeduced in `typename P::pointer`, which names it, and the
// conversion sequences pick the overload (LLVM's ilist
// `NodeAccess::getNodePtr<OptionsT>` with assertions on).
#include <cstdio>

struct NA {
  template <class P> static int get(typename P::pointer) { return 1; }
  template <class P> static int get(typename P::const_pointer) { return 2; }
};
struct Opt {
  typedef int *pointer;
  typedef const int *const_pointer;
};

template <class O> struct SNA : NA {
  using pointer = typename O::pointer;
  using const_pointer = typename O::const_pointer;
  static int node(pointer n) { return NA::get<O>(n); }
  static int node(const_pointer n) { return NA::get<O>(n); }
};

int main() {
  int i = 0;
  const int *c = &i;
  std::printf("%d %d %d %d\n", NA::get<Opt>(&i), NA::get<Opt>(c),
              SNA<Opt>::node(&i), SNA<Opt>::node(c));
}
