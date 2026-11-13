// [expr.new]/19, [dcl.init.ref]/5: placement arguments are passed as in a
// function call, so a reference-to-base parameter binds to an object of a
// derived class, including through an intermediate base and to a base that
// is not the first one. Reduced from downstream code: `new (allocator)
// T(...)` with a derived allocator and `operator new(size_t, Allocator&)`.
//@ expect: run
//@ stdout: 3 3 7
#include <cstddef>
#include <cstdio>

struct Allocator {
  virtual ~Allocator() {}
  int tag = 0;
};
struct Managed : Allocator {};
struct Sequential : Managed {};
struct Named {
  int pad[3];
};
struct Second : Named, Allocator {};

alignas(16) static unsigned char buf[64];
void *operator new(std::size_t, Allocator &a) {
  std::printf("%d", a.tag);
  return buf;
}

int main() {
  Sequential s;
  s.tag = 3;
  Second t;
  t.tag = 7;
  new (s) int;
  std::printf(" ");
  new (static_cast<Managed &>(s)) int;
  std::printf(" ");
  new (t) int;
  std::printf("\n");
}
