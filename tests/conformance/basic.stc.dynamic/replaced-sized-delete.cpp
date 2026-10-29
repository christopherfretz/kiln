// [basic.stc.dynamic.deallocation], [expr.delete]: with sized deallocation
// (the C++14 default), deleting a complete object calls the replaceable
// sized operator delete with the object's size.
//@ expect: run
//@ stdout: sized 24 | sized 8 | frees 2
extern "C" int printf(const char*, ...);
extern "C" void free(void*);

int frees;

struct T { long a, b, c; };

void operator delete(void* p, unsigned long n) noexcept {
  ++frees;
  printf("sized %lu | ", n);
  free(p);
}

int main() {
  T* t = new T{1, 2, 3};
  double* d = new double(1.0);
  delete t;
  delete d;
  printf("frees %d\n", frees);
}
