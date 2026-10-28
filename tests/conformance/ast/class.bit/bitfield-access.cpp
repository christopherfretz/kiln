//@ expect: ast
struct Flags {
  unsigned a : 3;
  int b : 5;
  unsigned : 0;
  bool c : 1;
};
int touch(Flags f, bool which) {
  f.a = 5;
  ++f.b;
  f.b += 2;
  (which ? f.a : f.a) = 1;
  (f.a, f.b) = 3;
  return f.a + f.b + f.c;
}
