// comments, whitespace, ignored attributes
[[nodiscard, deprecated]] inline int f([[maybe_unused]] int a) {
  if (a) [[likely]] {
    return a + 1; // one more
  }
  return 0;
}
int (*keep_a)(int) = f;
