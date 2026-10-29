// [expr.rel] + [expr.eq] + [expr.arith.conv]: relational and equality
// operators after the usual arithmetic conversions (a negative int compared
// with an unsigned converts to a large unsigned value).
//@ expect: run
//@ stdout: 1 0 1 1 1 1
//@ stdout: 0 1 1 1
extern "C" int printf(const char*, ...);

int main() {
  int neg = -1;
  unsigned one = 1;
  long lneg = -1;
  double half = 0.5;
  int arr[2] = {};
  printf("%d %d %d %d %d %d\n", 3 < 4, 3 >= 4, 2 == 2.0, half != 0, neg < (int)one,
         lneg < (long)one);
  printf("%d %d %d %d\n", neg < (int)one ? (unsigned)neg < one : 0, (unsigned)neg > one,
         &arr[0] < &arr[1], &arr[1] == arr + 1);
}
