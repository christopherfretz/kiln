// Debug info (WP-941): the `cleanup.dest` load and switch that leave a
// cleanup with several exits, and the shared `unreachable` default block,
// have no `!dbg` (clang's PopCleanupBlock appends them to the exit block
// directly, and getUnreachableBlock creates its terminator without the
// builder); gcov line counts depend on which instructions are located.
//@ args: -g
struct D {
  ~D();
};
int f(int x) {
  D d;
  if (x)
    return 1;
  for (;;) {
    D e;
    if (x > 2)
      break;
    if (x > 3)
      return 2;
  }
  return 0;
}
int g(int x) {
  switch (x) {
  case 1: {
    D a;
    if (x)
      return 3;
    break;
  }
  default:
    return x;
  }
  return 0;
}
