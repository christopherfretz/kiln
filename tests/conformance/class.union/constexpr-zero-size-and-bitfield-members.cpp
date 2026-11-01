// [class.union]/2, [expr.const]/5.10, GNU zero-length arrays: in a constant
// evaluation the member at an offset is the one whose storage contains it,
// not a zero-size member at the same offset, and a bit-field in a struct
// member of a union is reached through its enclosing members. This is the
// shape of libc++'s short `std::basic_string` (an anonymous struct of
// bit-fields, then `char __padding_[0]` before `__data_`).
//@ std: c++26
struct Short {
  struct {
    unsigned char is_long : 1;
    unsigned char size : 7;
  };
  char pad[0];
  char data[7];
};
struct Long {
  unsigned long cap;
};
union Rep {
  Short s;
  Long l;
};

constexpr int f() {
  Rep r{};
  r.s = Short();
  r.s.size = 3;
  r.s.data[0] = 'a';
  r.s.data[1] = 'b';
  char *p = r.s.data;
  p[2] = 'c';
  return r.s.size * 1000 + (r.s.data[0] - 'a') * 100 + (r.s.data[1] - 'a') * 10 +
         (p[2] - 'a') + r.s.is_long;
}
static_assert(f() == 3012);

struct Z {
  int pad[0];
  int x;
};
constexpr int g() {
  Z z{};
  int *p = &z.x;
  *p = 4;
  return z.x;
}
static_assert(g() == 4);
