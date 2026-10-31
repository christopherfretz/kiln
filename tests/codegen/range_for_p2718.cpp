// P2718R0 (C++23): the temporaries of a for-range-initializer live until
// the end of the loop, so `T` and the `L` returned through `id` are
// destroyed after the last iteration, in reverse order of construction;
// a temporary in an init-statement is not extended.
struct T {
  int v;
  T(int);
  ~T();
};
struct L {
  int a[2];
  ~L();
  const int* begin() const { return a; }
  const int* end() const { return a + 2; }
};
L make(const T&);
const L& id(const L&);
int use(int);

int extended() {
  int s = 0;
  for (int x : id(make(T(1)))) s += use(x);
  return s;
}
int direct() {
  int s = 0;
  for (int x : make(T(2))) s += x;
  return s;
}
int init_stmt() {
  int s = 0;
  for (int k = T(3).v; int x : make(T(k))) s += x;
  return s;
}
