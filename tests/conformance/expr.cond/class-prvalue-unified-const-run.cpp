// [expr.cond]/4.3, /7: a class prvalue arm unified with a `const` class
// lvalue arm of the same class is converted to the `const` type, so both
// arms have the same type and the result is a `const V` prvalue (clang's
// `NoOp` cast in `TryClassUnification`, then `hasSameType`). `auto &`
// binds to it as `const V &` and extends its lifetime. WP-799.
//@ expect: run
//@ strict-diags
//@ stdout: 7 9|1 2|ce 16
extern "C" int printf(const char *, ...);

struct V {
  int x;
  constexpr int get() const { return x; }
};

template <class T> struct is_const { static constexpr bool value = false; };
template <class T> struct is_const<const T> { static constexpr bool value = true; };

V make(int x) { return V{x}; }
const V cv{9};
const V &ref() { return cv; }

static_assert(is_const<decltype(true ? make(1) : ref())>::value);
static_assert(is_const<decltype(true ? ref() : make(1))>::value);
static_assert(!is_const<decltype(true ? make(1) : make(2))>::value);

constexpr V cmake(int x) { return V{x}; }
constexpr const V cc{10};

constexpr int sum(bool b) {
  auto &a = b ? cmake(6) : cc;
  auto &c = !b ? cmake(6) : cc;
  return a.get() + c.get();
}
static_assert(sum(true) == 16 && sum(false) == 16);

int main() {
  bool t = make(1).x == 1;
  auto &a = t ? make(7) : ref();
  auto &b = !t ? make(7) : ref();
  printf("%d %d|", a.get(), b.get());
  const V &c = t ? ref() : make(2);
  V d = !t ? ref() : make(2);
  printf("%d %d|", c.x == 9, d.x);
  printf("ce %d\n", sum(true));
}
