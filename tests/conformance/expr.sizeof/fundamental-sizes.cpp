// [expr.sizeof] + [basic.fundamental]: sizeof of the fundamental types and
// of arrays and aggregates (LP64 Linux targets).
//@ expect: run
//@ stdout: 1 2 4 8 8 4 8 1
//@ stdout: 40 12 8 3 16
extern "C" int printf(const char*, ...);

struct S {
  char c;
  int i;
  short s;
};
struct P {
  int* p;
};

int main() {
  printf("%zu %zu %zu %zu %zu %zu %zu %zu\n", sizeof(char), sizeof(short), sizeof(int),
         sizeof(long), sizeof(long long), sizeof(float), sizeof(double), sizeof(bool));
  int arr[10];
  char str[] = "ab";
  printf("%zu %zu %zu %zu %zu\n", sizeof arr, sizeof(S), sizeof(P), sizeof str,
         sizeof(__int128));
}
