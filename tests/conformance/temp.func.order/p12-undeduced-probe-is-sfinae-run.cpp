// [temp.deduct.partial]/12, [temp.deduct.general]/8: the check that a
// template parameter left without a value is named by the compared types
// is part of deduction, so a qualifier the probe cannot form
// (`typename O::pointer` with `O` given no class type) fails it quietly,
// never a hard error. Both templates are viable for `f<opts>(&x)`;
// neither is more specialized, so the conversion sequences decide (an
// identity over a qualification conversion, [over.ics.rank]/3.2.5).
// LLVM's `NodeAccess::getNodePtr<OptionsT>` overloads (ilist_node.h,
// YAMLParser.cpp). Verified against clang 19.
//@ expect: run
//@ strict-diags
//@ std: c++17
//@ stdout: 1 2 1
#include <cstdio>

struct opts {
  using pointer = int*;
  using const_pointer = const int*;
};

template <class O> int f(typename O::pointer) { return 1; }
template <class O> int f(typename O::const_pointer) { return 2; }

struct NodeAccess {
protected:
  template <class O> static int get(typename O::pointer) { return 1; }
  template <class O> static int get(typename O::const_pointer) { return 2; }
};
template <class O> struct Specific : NodeAccess {
  static int get(typename O::const_pointer p) { return NodeAccess::get<O>(p); }
  static int get(typename O::pointer p) { return NodeAccess::get<O>(p); }
};

int main() {
  int x = 0;
  const int* cp = &x;
  std::printf("%d %d %d\n", f<opts>(&x), f<opts>(cp), Specific<opts>::get(&x));
}
