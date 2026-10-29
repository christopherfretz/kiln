// [expr.add]: additive operators on integers (unsigned arithmetic wraps
// modulo 2^N) and pointer subtraction.
//@ expect: run
//@ stdout: 7 -1 4294967295 0
//@ stdout: 3
extern "C" int printf(const char*, ...);

int main() {
  int a = 3, b = 4;
  unsigned u = 0;
  unsigned max = 4294967295u;
  int arr[5] = {};
  int* p = &arr[1];
  int* q = &arr[4];
  printf("%d %d %u %u\n", a + b, a - b, u - 1, max + 1);
  printf("%ld\n", (long)(q - p));
}
