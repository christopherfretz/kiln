// [class.mem]/10, [expr.const]/2: a `const` integral static data member
// named in a default member initializer is usable in constant
// expressions; clang folds the read (`store i32 5`, and `d + 1` to 7)
// instead of loading the member, in every constructor that uses the
// initializer.
struct S {
  static const int c = 5;
  static constexpr int d = 6;
  int m = c;
  int n = d + 1;
  S() {}
  S(int) : m(c + d) {}
};
int f() { S s; S t(1); return s.m + s.n + t.m; }
struct I {
  static const int c = 2;
  int m = c * 3;
};
int g() { I i; return i.m; }
template <class T> struct U {
  static const T k = 9;
  T a = k;
  U() {}
};
int h() { return U<int>().a; }
