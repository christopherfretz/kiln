// [dcl.fct]/22, [temp.inst]/3: a function parameter declared before the
// first placeholder parameter of an abbreviated function template is a
// parameter of each specialization: its uses in the body refer to the
// specialization's parameter (libc++ formatter.handle.pass.cpp,
// `auto format(color c, auto& ctx) const`).
//@ expect: run
//@ stdout: red 3 9 11 gold
extern "C" int printf(const char*, ...);
const char* names[] = {"black", "red", "gold"};
struct B {
  const char* format(const char* p, int& ctx) const { ctx += 2; return p; }
};
struct F : B {
  auto format(int c, auto& ctx) const { return B::format(names[c], ctx); }
  int twice(int c, auto& ctx) const;
};
int F::twice(int c, auto& ctx) const { return c + c + ctx; }
int add(int c, short d, auto... xs) { return c + d + (xs + ... + 0); }
auto pick(int c, auto) { return [c] { return names[c]; }; }

int main() {
  F f;
  int x = 1;
  const char* s = f.format(1, x);
  long y = 5;
  printf("%s %d %d %d %s\n", s, x, f.twice(2, y), add(3, 4, x, 1), pick(2, 0.5)());
}
