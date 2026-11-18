// [expr.delete]/6, [basic.def.odr]/8: a non-dependent delete-expression
// in a member of a class template odr-uses the destructor in each
// instantiation, so the implicit destructor of the deleted class is
// defined (it destroys a member of that class template). Found running
// GCC's C++ testsuite, where kiln failed to link (WP-1645).
//@ expect: run
//@ stdout: 1
#include <cstdio>

template <class U> struct Holder {
  ~Holder();
};
struct Node {
  long id;
  Holder<Node> h;
};
Node *gone = nullptr;
int dtors = 0;
template <class U> Holder<U>::~Holder() {
  ++dtors;
  delete gone;
}
template struct Holder<Node>;

int main() {
  { Holder<Node> one; }
  std::printf("%d\n", dtors);
}
