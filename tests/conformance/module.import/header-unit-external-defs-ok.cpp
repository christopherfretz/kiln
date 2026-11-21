// [module.import]/6: the definitions a header unit may contain. The test
// file is built as a header unit (`as-header-unit`). As clang's check, a
// variable without an initializer, an `extern` variable (also with one),
// an inline, constexpr, consteval, deleted, defaulted or in-class
// function, an internal entity, a templated entity, an explicit
// instantiation, a variable template or static data member
// specialization, and a static data member declared in its class are not
// diagnosed.
//@ expect: compile-ok
//@ std: c++20
//@ strict-diags
//@ as-header-unit
inline int g() { return 2; }
inline int p1();
int p1() { return 1; }
static int s() { return 3; }
constexpr int cf() { return 13; }
consteval int cev() { return 1; }
int v;
extern int ev;
extern const int ec = 1;
extern "C" int ecv;
extern "C++" int ecpp;
inline int iv = 5;
static int sv = 6;
const int cv = 7;
constexpr int ce = 1;
namespace {
int anon() { return 8; }
int anon_v = 1;
} // namespace
struct S {
  int m() { return 9; }
  static int sm;
  static inline int si = 1;
  static constexpr int k = 1;
  friend void fr() {}
  S() = default;
  S(const S &) = delete;
  inline void h();
};
int S::sm;
void S::h() {}
void del() = delete;
template <class T> int tf(T) { return 11; }
template <class T> int vt = 1;
template <> int vt<int> = 2;
template <class T> struct X {
  static int m;
  void f();
};
template <class T> int X<T>::m = 1;
template <class T> void X<T>::f() {}
template <> int X<int>::m = 3;
template struct X<long>;
template int tf<long>(long);
struct P {
  int a, b;
};
inline P pp{1, 2};
static auto [b1, b2] = pp;
