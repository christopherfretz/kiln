// attributes not on O-10's list, a precondition, in this unit only
[[vendor::x, deprecated]] [[gnu::section(".text.f")]] inline int f(int a)
  pre(a > 0) post(r : r > 0) {
  return a;
}
int (*keep_a)(int) = f;
