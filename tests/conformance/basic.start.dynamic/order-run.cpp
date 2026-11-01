// [basic.start.dynamic]/1,2, [basic.start.term]/1, [class.temporary]/6:
// ordered dynamic initialization runs in definition order before main;
// objects are destroyed in the reverse order of their construction
// completing; an inline variable and a template static data member are
// initialized once; a temporary bound to a static reference lives until
// exit. (Where the unordered and partially-ordered initializations of
// `Tm<int>::m` and `ia` run is unspecified; the expectation is the
// Itanium/clang placement: their llvm.global_ctors entries precede the
// file's ordered initializers.)
//@ expect: run
//@ stdout: I6 T7 A1 g=11 A2 R3 M5 main 1 11 2 7 6 3 5 48 ~R3 ~A2 ~A1 ~T7 ~I6
extern "C" int printf(const char*, ...);

int live = 0; // constant-initialized: usable by every dynamic initializer
struct A {
  int v;
  A(int x) : v(x) { ++live; printf("%s%d ", tag(), v); }
  ~A() { printf(--live ? "~%s%d " : "~%s%d\n", tag(), v); }
  const char *tag() const { return v == 7 ? "T" : v == 6 ? "I" : v == 3 ? "R" : "A"; }
};
int f(int x) { return x + 10; }
A a1(1);
int g = f(1);
int pg = printf("g=%d ", g);
A a2(2);
template <class T> struct Tm { static A m; };
template <class T> A Tm<T>::m(7);
inline A ia(6);
const A &ra = A(3);
struct M { int v; M(int x) : v(x) { printf("M%d ", v); } };
M m(5);
int sum = a1.v + g + a2.v + Tm<int>::m.v + ia.v + ra.v + m.v;

int main() {
  printf("main %d %d %d %d %d %d %d %d ", a1.v, g, a2.v, Tm<int>::m.v, ia.v, ra.v, m.v, sum + 13);
  return 0;
}
