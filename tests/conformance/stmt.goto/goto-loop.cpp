// [stmt.goto] + [stmt.label]: goto transfers control to a label in the same
// function, backwards or forwards.
//@ expect: run
//@ stdout: 3 2 1 done
extern "C" int printf(const char*, ...);

int main() {
  int n = 3;
again:
  if (n == 0) goto done;
  printf("%d ", n);
  --n;
  goto again;
done:
  printf("done\n");
}
