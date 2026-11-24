// [temp.spec.partial.general]/8, [temp.spec.partial.match], [temp.expl.spec]:
// a non-type argument written as a call in a partial or explicit
// specialization of a class or variable template matches by its value.
// Kiln keeps the written arguments for tooling clients (spec/ast.md 8.3
// "Arguments as written"); matching and ordering use the converted ones.
// Reduced from real code.
//@ expect: run
//@ stdout: 1 2 0 3 4 0 6
extern "C" int printf(const char *, ...);

constexpr int f(int x) { return x; }

template <class T, int N> struct P { static constexpr int k = 0; };
template <class T> struct P<T, f(1)> { static constexpr int k = 1; };
template <> struct P<int, f(2)> { static constexpr int k = 2; };

template <class T, int N> constexpr int v = 0;
template <class T> constexpr int v<T, f(3)> = 3;
template <> constexpr int v<int, f(4)> = 4;

template <int N> int fh() { return 0; }
template <> int fh<f(6)>() { return 6; }

int main() {
  printf("%d %d %d %d %d %d %d\n", P<char, 1>::k, P<int, 2>::k, P<int, 3>::k,
         v<char, 3>, v<int, 4>, v<int, 5>, fh<6>());
}
