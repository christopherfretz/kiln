// [over.ics.rank]/3.2 with CWG2352 (clang's resolution): binding a
// `const T *const &` directly to a `T *` counts as a qualification
// adjustment, so an identity binding (`T *const &`) beats it, a
// less-qualified similar binding beats a more-qualified one, and the
// lvalue binding beats binding an rvalue reference to a converted
// temporary; partial ordering still prefers the more specialized
// `T *const &`.
//@ expect: compile-ok
int *gp;
constexpr int f1(int *const &) { return 1; }
constexpr int f1(const int *const &) { return 2; }
static_assert(f1(gp) == 1);
constexpr int f3(const int *const &) { return 1; }
constexpr int f3(const volatile int *const &) { return 2; }
static_assert(f3(gp) == 1);
constexpr int f4(const int *const &) { return 1; }
constexpr int f4(int *const &&) { return 2; }
static_assert(f4(gp) == 1);
constexpr int f5(const int *const &) { return 1; }
constexpr int f5(const int *&&) { return 2; }
static_assert(f5(gp) == 1);
constexpr int f6(const int *const &) { return 1; }
constexpr int f6(...) { return 2; }
static_assert(f6(gp) == 1);
template <class T> constexpr int f7(const T *const &) { return 1; }
template <class T> constexpr int f7(T *const &) { return 2; }
static_assert(f7(gp) == 2);
