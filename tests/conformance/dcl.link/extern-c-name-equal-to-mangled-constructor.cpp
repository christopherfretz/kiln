// An `extern "C"` function whose name is the Itanium mangled name of a
// declared, never-defined constructor: the constructor call resolves to
// that symbol at link time (clang 22 and GCC 16.1 both print "called").
// Found running GCC's C++ testsuite.
//@ expect: run
//@ stdout: called
//@ xfail(kiln): WP-1663: codegen stops with "two non-local globals for one symbol" when a C-linkage function's name equals a C++ mangled name
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
