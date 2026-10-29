// A complete program: main with printf output and an exit status.
extern "C" int printf(const char*, ...);
struct Acc { long total; int count; };
Acc make(long total, int count);
long avg(Acc a) { return a.count ? a.total / a.count : 0; }
int main() {
  long total = 0;
  for (int i = 1; i <= 10; i++) total += i * i;
  printf("total=%ld avg=%ld half=%.2f\n", total, avg(make(total, 10)), total / 2.0);
  return total == 385 ? 0 : 1;
}
