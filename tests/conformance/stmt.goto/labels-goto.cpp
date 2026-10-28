//@ expect: compile-ok
int loop(int n) {
  int i = 0;
top:
  if (i >= n) goto done;
  ++i;
  goto top;
done:
  return i;
}
