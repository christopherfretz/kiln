// [class.static.data]: one object shared by every instance, named through
// the class or through an object; enumerators and nested types of a class.
//@ expect: run
//@ stdout: 4 1 13 13
//@ xfail(kiln): needs WP-065 parser classes
extern "C" int printf(const char*, ...);
struct S {
  int x;
  static int count;
  static const int k = 3;
  static constexpr long big = 1L << 40;
  enum E { e1, e2 };
  struct In { int y; };
  static int bump() { return ++count; }
};
int S::count = 5;
int main() {
  S s, t;
  s.x = 4;
  S::In in;
  in.y = S::e2;
  S::count += s.count + t.e1 + S::k + (S::big >> 40) - 1;
  s.bump();
  S::bump();
  printf("%d %d %d %d\n", s.x, in.y, t.count - 2, S::count - 2);
}
