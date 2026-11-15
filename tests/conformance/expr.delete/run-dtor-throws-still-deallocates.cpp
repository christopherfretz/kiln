// [expr.delete]/7: the deallocation function is called even when the
// destructor throws, for a non-virtual destructor and for `::delete`
// through a virtual one (which calls the complete destructor, then the
// global deallocation function). Found running GCC's C++ testsuite.
//@ expect: run
//@ strict-diags
//@ stdout: 1 2 1
extern "C" int printf(const char*, ...);
using size_t = decltype(sizeof 0);
int freed;
alignas(16) unsigned char pool[64];
void* operator new(size_t) { return pool; }
void operator delete(void* p) noexcept { freed = p == pool ? 1 : -1; }
void operator delete(void* p, size_t) noexcept { freed = p == pool ? 1 : -1; }
struct Plain {
  ~Plain() noexcept(false) { throw 1; }
};
struct Virt {
  virtual ~Virt() noexcept(false) { throw 2; }
  void operator delete(void*) noexcept { freed = 2; }
};
int main() {
  Plain* p = new Plain;
  try { delete p; } catch (int) {}
  printf("%d ", freed);
  freed = 0;
  Virt* v = new Virt;
  try { delete v; } catch (int) {}
  printf("%d ", freed);
  freed = 0;
  v = ::new Virt;
  try { ::delete v; } catch (int) {}
  printf("%d\n", freed);
}
