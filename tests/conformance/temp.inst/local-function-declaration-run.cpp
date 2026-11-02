// [temp.inst]/3, [basic.scope.block]: block-scope function declarations
// (including an operator and a constexpr one) in function templates are
// declared again in each instantiation and name the namespace-scope
// functions; a local class's friend redeclares the instantiated operator
// ([class.friend]/11).
//@ expect: run
struct A { int v; };
int g(int x) { return x + 1; }
int g(long x) { return x + 2; }
bool operator==(A a, A b) { return a.v == b.v; }
template <class T> int t(T v) {
  int g(T);
  bool operator==(A, A);
  struct L {
    friend bool operator==(A, A);
    static int call(T x) { return g(x); }
  };
  A a{1}, b{1};
  return g(v) + L::call(v) + (a == b);
}
template <class T> constexpr int c(T v) {
  constexpr int k(T);
  return k(v);
}
constexpr int k(int x) { return x * 3; }
static_assert(c(2) == 6);
int main() { return t(1) + t(1L) == 2 + 2 + 1 + 3 + 3 + 1 ? 0 : 1; }
