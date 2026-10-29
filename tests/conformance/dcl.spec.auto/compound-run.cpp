// [dcl.type.auto.deduct]: auto with pointer and reference declarators
// deduces through the declarator; references alias, pointers point.
//@ expect: run
//@ stdout: 7 7 8 9 9
extern "C" int printf(const char*, ...);

int g = 5;
auto &ref_g() { return g; }
auto *ptr_g() { return &g; }

int main() {
  int x = 7;
  auto &r = x;
  auto *p = &x;
  auto &&f = x;
  printf("%d %d ", r, *p);
  f = 8;
  printf("%d ", x);
  ref_g() = 9;
  printf("%d %d\n", g, *ptr_g());
  return 0;
}
