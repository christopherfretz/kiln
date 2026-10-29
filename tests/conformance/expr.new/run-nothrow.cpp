// [expr.new]: the non-throwing form `new (std::nothrow) T` calls the nothrow
// allocation function; the result is null-checked before initialization.
//@ expect: run
//@ stdout: ok 11 | ok 0 0 0
namespace std { struct nothrow_t {}; extern const nothrow_t nothrow; }
void* operator new(unsigned long, const std::nothrow_t&) noexcept;
void* operator new[](unsigned long, const std::nothrow_t&) noexcept;
extern "C" int printf(const char*, ...);

int main() {
  int* p = new (std::nothrow) int(11);
  if (p) printf("ok %d | ", *p);
  int* a = new (std::nothrow) int[3]();
  if (a) printf("ok %d %d %d\n", a[0], a[1], a[2]);
  delete[] a;
  delete p;
}
