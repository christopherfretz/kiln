// An `extern "C"` function whose name is the Itanium mangled name of a
// declared, never-defined constructor: the constructor call resolves to
// that symbol at link time (clang 22 and GCC 16.1 both print "called"):
// one global per symbol (spec/codegen.md, WP-1663).
// Found running GCC's C++ testsuite.
//@ expect: run
//@ stdout: called
#include <cstdio>

struct Gadget {
  Gadget(int *);
};

extern "C" void _ZN6GadgetC1EPi(...) { std::puts("called"); }

int main() {
  int slot = 0;
  Gadget g(&slot);
  (void)g;
}
