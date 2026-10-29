// [basic.stc.dynamic.allocation]: allocating an object whose alignment
// exceeds __STDCPP_DEFAULT_NEW_ALIGNMENT__ calls the align_val_t overload, so
// the storage is suitably aligned.
//@ expect: run
//@ stdout: aligned aligned
extern "C" int printf(const char*, ...);

struct alignas(64) Big { int v; };

int main() {
  Big* p = new Big{1};
  Big* a = new Big[3]{};
  printf("%s ", (unsigned long)p % 64 == 0 ? "aligned" : "misaligned");
  printf("%s\n", (unsigned long)a % 64 == 0 ? "aligned" : "misaligned");
  delete[] a;
  delete p;
}
