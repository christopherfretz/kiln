// [stmt.if]: an if statement with an init-statement; the declared variable
// is in scope in both branches.
//@ expect: run
//@ stdout: even 4 | odd 3 | done
extern "C" int printf(const char*, ...);

int twice(int v) { return v * 2; }

int main() {
  for (int i = 2; i <= 3; ++i) {
    if (int t = twice(i); t % 4 == 0)
      printf("even %d | ", t);
    else
      printf("odd %d | ", t - i);
  }
  if (int z = 0)
    printf("unreachable %d\n", z);
  else
    printf("done\n");
}
