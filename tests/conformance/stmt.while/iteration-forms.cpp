// [stmt.while] + [stmt.do] + [stmt.for]: the three loop forms, including a
// do-while whose body runs once although the condition starts false.
//@ expect: run
//@ stdout: 0 1 2 3 4
//@ stdout: once
//@ stdout: 55 10
extern "C" int printf(const char*, ...);

int main() {
  int i = 0;
  while (i < 5) printf(i < 4 ? "%d " : "%d\n", i++);
  int n = 0;
  do printf("once\n");
  while (n++ > 0);
  int sum = 0, k;
  for (k = 1; k <= 10; ++k) sum += k;
  printf("%d %d\n", sum, k - 1);
}
