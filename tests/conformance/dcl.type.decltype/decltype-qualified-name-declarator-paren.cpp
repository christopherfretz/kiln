// [dcl.type.decltype]/1.3, [dcl.ambig.res]/1, [basic.lookup.qual]/1: in
// `T d(decltype(v)::name ...)`, whether the parenthesized list declares
// parameters or is an initializer depends on what `name` is in the class
// of `v`: a type makes a function declaration, a value an initializer.
// Variable, parameter, data member and enumerator operands; the class must
// be defined. WP-843, WP-851.
//@ expect: run
//@ stdout: 7 5 2 9 1
extern "C" int printf(const char*, ...);
struct M {
  using key_type = int;
  static const int value = 5;
  enum E { e1 = 1, e2 = 2 };
};
M m;
void g(decltype(m)::key_type);              // function
int v(decltype(m)::value);                  // variable, = 5
int w(decltype(m)::e2);                     // variable, = 2
void g(int k) { printf("%d ", k); }
struct H {
  M mem;
  int h(decltype(mem)::key_type k) { return k; }
};
int p(M param, decltype(param)::key_type);  // function
int p(M, int k) { return k; }
enum class Color { red = 1 };
constexpr Color c = Color::red;
Color q(decltype(c)::red);                  // variable (an enumerator)
int main() {
  g(7);
  printf("%d %d %d %d\n", v, w, H{}.h(9), int(q));
  return p(M{}, 0);
}
