// [expr.add]/4: a pointer into an array member that shares its offset with
// a zero-size member before it (libc++'s short string: `char
// __padding_[0]` then `char __data_[N]`) ranges over that array.
// spec/oven.md "Pointers".
//@ expect: compile-ok
struct S {
  char pad[0];
  char d[4];
};
constexpr int walk() {
  S s{};
  char *p = s.d;
  p += 3;
  *p = 7;
  return s.d[3] + int(p - s.d);
}
static_assert(walk() == 10);

constexpr int move() {
  S s{};
  const char src[4] = {1, 2, 3, 4};
  __builtin_memmove(s.d, src, 4);
  return s.d[0] + s.d[3];
}
static_assert(move() == 5);
