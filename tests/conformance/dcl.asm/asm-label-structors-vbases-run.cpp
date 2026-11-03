// [dcl.asm] (GNU asm labels) on the structors of classes with virtual
// bases (WP-492): every variant takes the label, and the label names the
// complete-object variant (no VTT parameter; clang replaces the
// base-object definition by it), which constructs the virtual base. A
// labelled virtual destructor's deleting variant is unreachable (clang
// appends it to the label's function), so `delete` through the vtable
// runs the destructor body alone. Destroying a labelled class with
// virtual bases, or deriving from one, calls the label as the
// base-object variant (clang's output recurses or crashes there), so
// neither is done here.
//@ expect: run
//@ stdout: B D 7 | B W 7 | V ~V
#include <cstdio>
struct B { int b = 7; B() { std::printf("B "); } };
struct D : virtual B {
  D() asm("ctor_d");
  ~D() asm("dtor_d");
};
D::D() { std::printf("D "); }
D::~D() {}
struct W : virtual B { W() asm("ctor_w"); virtual ~W() asm("vdtor_w"); };
W::W() { std::printf("W "); }
W::~W() {}
struct V { V() { std::printf("V "); } virtual ~V() asm("vdtor"); int v; };
V::~V() { std::printf("~V"); }
int main() {
  D *d = new D;
  std::printf("%d | ", d->b);
  W *w = new W;
  std::printf("%d | ", w->b);
  V *v = new V;
  delete v;
  std::printf("\n");
}
