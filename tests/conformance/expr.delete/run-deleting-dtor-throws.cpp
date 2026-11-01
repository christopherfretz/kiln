// [expr.delete]/6, [class.dtor]/16: a delete-expression through a virtual
// destructor that throws still calls the deallocation function (the
// deleting destructor runs it as a cleanup), then the exception propagates.
//@ expect: run
//@ strict-diags
using size_t = decltype(sizeof 0);
int deletes = 0;
int dtors = 0;
alignas(16) unsigned char pool[64];

struct B {
  virtual ~B() noexcept(false) { ++dtors; if (fail) throw 7; }
  void *operator new(size_t) { return pool; }
  void operator delete(void *p, size_t n) { if (p == pool && n >= sizeof(B)) ++deletes; }
  bool fail = false;
};
struct D : B {
  ~D() noexcept(false) override { ++dtors; }
};

int main() {
  B *b = new D;
  b->fail = true;
  int caught = 0;
  try {
    delete b;
  } catch (int v) {
    caught = v;
  }
  if (caught != 7 || dtors != 2 || deletes != 1) return 1;
  b = new D;
  delete b;
  if (dtors != 4 || deletes != 2) return 2;
  return 0;
}
