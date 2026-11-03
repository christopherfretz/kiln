//@ expect: run
//@ requires: clang>=23
//@ oracle: none
//@ stdout: ok
// [basic.indet]/1, [dcl.attr.indet] (C++26, P2795R5): a variable or
// parameter declared `[[indeterminate]]` keeps an indeterminate value (no
// erroneous value is written: spec/codegen.md §7r); reading it is
// undefined, so only the program's well-defined part is checked.
extern "C" int printf(const char *, ...);
struct T {
  int v[4];
};
[[gnu::noinline]] int g(int p [[indeterminate]]) {
  p = 1;
  return p;
}
int main() {
  int x [[indeterminate]];
  T t [[indeterminate]];
  unsigned char buf [[indeterminate]][64];
  x = 2;
  t.v[0] = 3;
  buf[0] = 4;
  printf(g(0) + x + t.v[0] + buf[0] == 10 ? "ok\n" : "bad\n");
}
