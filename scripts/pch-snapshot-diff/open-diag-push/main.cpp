int quiet() {
  int unused_in_push = 1;
  return 0;
}
#pragma clang diagnostic pop
int loud(int x) {
  int unused_after_pop = 1;
  { int x = 2; (void)x; }
  x == 1;
  return 0;
}
#pragma clang diagnostic pop
