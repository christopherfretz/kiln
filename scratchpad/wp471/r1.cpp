struct S {
  int a;
  static_assert(sizeof(S) == 4);
  static_assert(alignof(S) == 4);
  static_assert(__builtin_offsetof(S, a) == 0);
  static constexpr S *p = nullptr;
  static_assert(!p);
  long b;
};
static_assert(sizeof(S) == 2 * sizeof(long));
static_assert(__builtin_offsetof(S, b) == sizeof(long));
