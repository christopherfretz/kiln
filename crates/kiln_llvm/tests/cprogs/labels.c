// Computed goto: blockaddress and indirectbr.
int jump(int i) {
  static void *targets[] = {&&a, &&b};
  goto *targets[i];
a:
  return 1;
b:
  return 2;
}
