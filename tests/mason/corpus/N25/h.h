// an implicit destructor (constexpr from C++20) and a lambda (constexpr from C++17)
struct M {
  ~M() {}
};
struct X {
  M s;
  int v = 1;
};
inline int f(int a) {
  X x;
  auto l = [a, &x] { return a + x.v; };
  return l();
}
