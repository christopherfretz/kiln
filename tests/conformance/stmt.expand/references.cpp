// [stmt.expand]/5.1: with `auto&` or `auto&&` an enumerating element
// variable binds to an lvalue expression of the list (no copy, so a
// non-copyable type works and writes go to the original), and to a
// temporary for a prvalue (lifetime-extended to the element's scope).
//@ expect: run
//@ strict-diags
//@ requires: clang>=23
//@ stdout: 0 0 30 3
extern "C" int printf(const char*, ...);

struct NoCopy {
  int v;
  constexpr NoCopy(int v) : v(v) {}
  NoCopy(const NoCopy&) = delete;
};

constexpr int write() {
  int a = 1, b = 2;
  template for (auto&& r : {a, b}) { r = 0; }
  return a + b;
}
static_assert(write() == 0);

constexpr int no_copy() {
  NoCopy x{10}, y{20};
  int s = 0;
  template for (auto& r : {x, y}) { s += r.v; }
  template for (const auto& r : {NoCopy(1), NoCopy(2)}) { s += r.v; }
  return s;
}
static_assert(no_copy() == 33);

int main() {
  int a = 1, b = 2;
  template for (auto& r : {a, b}) { r = 0; }
  NoCopy x{10}, y{20};
  int s = 0;
  template for (auto& r : {x, y}) { s += r.v; }
  int n = 0;
  template for (auto&& t : {1, 2, 3}) { n += t > 0; }
  printf("%d %d %d %d\n", a, b, s, n);
  return 0;
}
