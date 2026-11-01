// [expr.new]/16, [expr.delete]/6: array new of a class with a destructor
// remembers the element count (an array cookie); delete[] destroys every
// element in reverse order and passes the full size to a sized class
// operator delete[]; a throwing constructor destroys the constructed
// elements and frees the storage.
//@ expect: run
//@ stdout: c0 c1 c2 d2 d1 d0 | c0 c1 c2 c3 c4 c5 d5 d4 d3 d2 d1 d0 | new 24 c0 c1 c2 c3 d3 d2 d1 d0 delete 24 | new 32 c0 c1 throw d1 d0 delete 32 caught
extern "C" int printf(const char*, ...);
extern "C" void* malloc(decltype(sizeof 0));
extern "C" void free(void*);

int ctors, limit = 100;
struct D {
  int id;
  D() : id(ctors++) {
    if (id == limit) {
      printf("throw ");
      throw 1;
    }
    printf("c%d ", id);
  }
  ~D() { printf("d%d ", id); }
};
struct S : D {
  void* operator new[](decltype(sizeof 0) n) {
    printf("new %d ", (int)n);
    return malloc(n);
  }
  void operator delete[](void* p, decltype(sizeof 0) n) {
    printf("delete %d ", (int)n);
    free(p);
  }
};

int count(int n) { return n; }

int main() {
  D* a = new D[3];
  delete[] a;
  printf("| ");
  ctors = 0;
  D (*m)[3] = new D[count(2)][3];
  delete[] m;
  printf("| ");
  ctors = 0;
  S* s = new S[count(4)];
  delete[] s;
  printf("| ");
  ctors = 0;
  limit = 2;
  try {
    S* t = new S[count(6)];
    delete[] t;
  } catch (int) {
    printf("caught\n");
  }
}
