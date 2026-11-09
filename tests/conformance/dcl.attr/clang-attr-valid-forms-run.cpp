// The valid forms of clang's `require_constant_initialization`,
// `annotate` and `unavailable` attributes and of `[[carries_dependency]]`
// (WP-954; [basic.start.static]/2, [dcl.attr.depend], [dcl.attr.grammar]/5):
// none changes what the program computes. A constant-initialized variable
// keeps its value, the attribute may be added on a later declaration
// before the initializing one, an unavailable entity may be declared and
// used inside another unavailable function, and carries_dependency may be
// repeated after the first declaration that has it.
//@ std: c++23
//@ expect: run
//@ strict-diags
//@ stdout: 42 7 3 9 12 1
extern "C" int printf(const char *, ...);

#define RCI __attribute__((require_constant_initialization))
struct Lit {
  constexpr Lit(int v) : v(v) {}
  int v;
};
constexpr int twice(int x) { return 2 * x; }

RCI int a = 42;
RCI Lit b(7);
RCI thread_local int t = 3;
extern int later;
RCI extern int later;
int later = 9;
[[clang::require_constant_initialization]] const int &ref = a;
struct M {
  RCI static int sm;
};
int M::sm = twice(6);
template <int N> struct TV {
  RCI static const int v;
};
template <int N> const int TV<N>::v = N;

constexpr int k = 2;
[[clang::annotate("plain")]] int ann_v = 1;
[[clang::annotate("args", 1, k, twice(k), "text")]] int ann_args() { return 0; }
struct [[clang::annotate("cls")]] AC {
  [[clang::annotate("member", sizeof(int))]] int x = 0;
};
template <int N> struct AT {
  [[clang::annotate("dep", N + 1)]] int f() { return N; }
};

void gone() __attribute__((unavailable("use other")));
inline void gone2() __attribute__((unavailable)) { gone(); }

[[carries_dependency]] int *dep([[carries_dependency]] int *p);
[[carries_dependency]] int *dep([[carries_dependency]] int *p);
int *dep(int *p) { return p; }
void pd([[carries_dependency]] int *);
void pd(int *) {}

int main() {
  static_assert(TV<3>::v == 3);
  int z = 0;
  pd(dep(&z));
  printf("%d %d %d %d %d %d\n", a, b.v, t, later, M::sm,
         ref + ann_v + ann_args() + AC().x + AT<5>().f() - 47);
}
