// [dcl.array]/1, [temp.type]/2: a constant bound written as a call is the
// same type as the bound written as its value. Kiln keeps the written
// expression as type sugar (spec/types.md 7.2 "Written bounds") for
// tooling clients; it changes no type identity: redeclarations, overload
// resolution, template argument deduction and specialization all see
// `int[2]`. Reduced from real code.
//@ expect: run
//@ stdout: 2 3 4 5 2 1
extern "C" int printf(const char *, ...);

constexpr int f(int x) { return x; }

extern int arr[2];
int arr[f(2)];

template <class T, class U> struct same { static constexpr bool v = false; };
template <class T> struct same<T, T> { static constexpr bool v = true; };
static_assert(same<int[f(2)], int[2]>::v);
static_assert(same<decltype(arr), int[2]>::v);
static_assert(sizeof(int[f(13)]) == sizeof(int[13]));

template <int N> int extent(int (&)[N]) { return N; }
int by_ref(int (&)[f(3)]) { return 3; }
int by_ref(int (&)[4]) { return 4; }

struct M { int m[f(4)]; };
template <class T> struct TA { T a[f(5)]; };

template <class T> struct pick { static constexpr int v = 0; };
template <> struct pick<int[f(2)]> { static constexpr int v = 1; };

int main() {
  int three[3] = {};
  int four[f(4)] = {};
  int (*p)[f(3)] = new int[f(2)][f(3)];
  TA<int> ta;
  printf("%d %d %d %d %d %d\n", extent(arr), by_ref(three), by_ref(four),
         static_cast<int>(sizeof(ta.a) / sizeof(int)),
         static_cast<int>(sizeof(M::m) / sizeof(int) / 2), pick<int[2]>::v);
  delete[] p;
}
