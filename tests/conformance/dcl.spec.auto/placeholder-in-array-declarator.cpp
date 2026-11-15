// [dcl.type.auto.deduct]/2.1: every occurrence of the placeholder in the
// declared type is replaced by the invented parameter, also below an array
// declarator (`auto (*p)[1]`, `auto (&r)[2]`), and deduced against the
// initializer as a function template parameter would be
// ([temp.deduct.type]/8 `T[i]`). Found running GCC's C++ testsuite.
//@ expect: compile-ok
template <class T> concept Int = __is_same(T, int);
int v[1];
int arr[2];
const int carr[3] = {};
int f(int);
struct C {
  int m;
  static int s[2];
};
auto (*p)[1] = &v;
static_assert(__is_same(decltype(p), int (*)[1]));
auto (&r)[2] = arr;
static_assert(__is_same(decltype(r), int (&)[2]));
const auto (&cr)[3] = carr;
static_assert(__is_same(decltype(cr), const int (&)[3]));
auto (&cr2)[3] = carr;
static_assert(__is_same(decltype(cr2), const int (&)[3]));
auto (*fp)(int) = f;
static_assert(__is_same(decltype(fp), int (*)(int)));
auto C::*pm = &C::m;
static_assert(__is_same(decltype(pm), int C::*));
const int *pi = nullptr;
const auto *const &x = pi;
static_assert(__is_same(decltype(x), const int *const &));
Int auto (*ip)[1] = &v;
static_assert(__is_same(decltype(ip), int (*)[1]));
auto *np = new auto(&v);
static_assert(__is_same(decltype(np), int (**)[1]));
int s2[2][2];
void g() {
  for (auto (&row)[2] : s2)
    static_assert(__is_same(decltype(row), int (&)[2]));
}
template <auto (*P)[1]> struct N {
  static_assert(__is_same(decltype(P), int (*)[1]));
};
N<&v> n;
struct S {
  static constexpr auto (*sp)[2] = &C::s;
};
static_assert(__is_same(decltype(S::sp), int (*const)[2]));

template <class T> void t(T x) {
  T a[2] = {x, x};
  auto (&ra)[2] = a;
  static_assert(__is_same(decltype(ra), T (&)[2]));
  auto (*pv)[1] = &v;
  static_assert(__is_same(decltype(pv), int (*)[1]));
}
template void t<long>(long);
template <int K> void u() {
  int a[K];
  auto (*pa)[K] = &a;
  static_assert(__is_same(decltype(pa), int (*)[K]));
}
template void u<4>();
