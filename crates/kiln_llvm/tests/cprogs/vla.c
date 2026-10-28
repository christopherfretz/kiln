// Variable-length arrays: stacksave/stackrestore and a counted alloca.
int last(int n) {
  int a[n];
  a[n - 1] = n;
  return a[n - 1];
}
