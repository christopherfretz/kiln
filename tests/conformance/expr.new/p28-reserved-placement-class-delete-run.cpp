// [expr.new]/28: `new (buf) T(...)` allocates with the reserved global
// placement `operator new(std::size_t, void*)`, and its matching
// deallocation function is looked up in T first, so a class-scope
// `operator delete(void*, void*)` is called with the object pointer and
// the placement argument (`buf`) when the initialization throws. gRPC's
// `ServerContextBase::CompletionOp` declares that pair beside a sized
// usual `operator delete`.
//@ expect: run
//@ stdout: placement delete 1 1
//@ stdout: caught 7
//@ stdout: no throw 5
#include <cstddef>
#include <cstdio>
#include <new>

struct Op {
  int v;
  explicit Op(int x) : v(x) {
    if (x == 7)
      throw x;
  }
  static void operator delete(void *, std::size_t) { std::puts("usual delete"); }
  static void operator delete(void *p, void *place);
};

alignas(Op) unsigned char buf[sizeof(Op)];

void Op::operator delete(void *p, void *place) {
  std::printf("placement delete %d %d\n", p == buf, place == buf);
}

int main() {
  try {
    new (buf) Op(7);
  } catch (int e) {
    std::printf("caught %d\n", e);
  }
  Op *o = new (buf) Op(5);
  std::printf("no throw %d\n", o->v);
  return 0;
}
