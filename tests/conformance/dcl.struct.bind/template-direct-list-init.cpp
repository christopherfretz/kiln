// [dcl.struct.bind]/1, [dcl.type.auto.deduct]/2.1: in `auto [a, b]{x}`
// the variable `e` is direct-list-initialized from `{x}`, so its type is
// that of `x`, not `std::initializer_list` -- also when the declaration
// is instantiated from a template (libc++ complex.tuple/get).
//@ expect: run
//@ strict-diags
struct C { int a, b; };
template <class T> int f() {
  T c{1, 2};
  auto [r, i]{c};
  auto& [x, y]{c};
  x = 5;
  return r + i + c.a + y;
}
int main() { return f<C>() == 1 + 2 + 5 + 2 ? 0 : 1; }
