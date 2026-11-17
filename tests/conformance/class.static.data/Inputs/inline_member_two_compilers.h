// Shared by the *-two-compilers-run.cpp tests: inline static data
// members (constant and dynamic initialization), the redundant
// redeclaration of a `constexpr` member, defined in objects of two
// compilers.
extern "C" int printf(const char *, ...);
int count();
int seven();
struct A {
  static constexpr int n = 7;
  static inline int m = count();
  static inline const char *s = "s";
};
constexpr int A::n;
template <class T> struct B { static inline T v = T(seven()) * 6; };
struct Units {
  const int *n;
  int *m;
  const char **s;
  int *v;
};
Units from_clang();
Units from_kiln();
