// [basic.stc.dynamic], [replacement.functions]: a program may replace the
// global allocation and deallocation functions; new/delete expressions then
// call the replacements, and the sized delete receives the allocated size.
//@ expect: run
//@ stdout: new 4 | new 40 | delete | delete | allocs 2 frees 2
extern "C" int printf(const char*, ...);
extern "C" void* malloc(unsigned long);
extern "C" void free(void*);

int allocs, frees;

void* operator new(unsigned long n) {
  ++allocs;
  printf("new %lu | ", n);
  return malloc(n);
}
void* operator new[](unsigned long n) {
  ++allocs;
  printf("new %lu | ", n);
  return malloc(n);
}
void operator delete(void* p) noexcept {
  ++frees;
  printf("delete | ");
  free(p);
}
void operator delete[](void* p) noexcept {
  ++frees;
  printf("delete | ");
  free(p);
}

int main() {
  int* a = new int(1);
  int* b = new int[10];
  delete a;
  delete[] b;
  printf("allocs %d frees %d\n", allocs, frees);
}
