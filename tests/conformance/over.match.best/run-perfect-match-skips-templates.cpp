// [over.match.best], clang 21+ (llvm-project's deferred template candidates,
// `OverloadCandidateSet::PerfectViableFunction`): when a non-template
// candidate is a perfect match (every argument, the object argument
// included, an identity standard conversion; a reference bound to the
// argument's own type, cv-qualification included, and no lvalue reference to
// an rvalue), the function templates named without explicit template
// arguments are not deduced at all, so a signature that would be a hard error
// to substitute (`bad<T>::type`) is never instantiated. The shape of
// a downstream ranking-traits header: `rank(BooleanYesNo)`
// beside `template<class T> enable_if<is_unsigned<T>, ...> rank(T)`, where
// boost::is_unsigned<enum> is ill-formed. For calls (lvalue-to-rvalue,
// array-to-pointer and function-to-pointer conversions are identities;
// default arguments and an unused ellipsis do not matter), member calls (an
// lvalue object for a member without ref-qualifier, a prvalue one for `&&`,
// an explicit object parameter, the implicit `this`), operators (a rewritten
// `operator<=>` candidate too), `operator()`, constructors, and range-based
// `for`'s `begin`/`end` found by argument-dependent lookup. Of two perfect
// candidates, the more constrained one is picked.
//@ expect: run
//@ requires: clang>=22
//@ exit: 0
template <class T> struct bad {
  static_assert(sizeof(T) == 0, "instantiated");
  typedef int type;
};
enum YN { NO, YES };

int rank(YN) { return 1; }
template <class T> typename bad<T>::type rank(T) { return 0; }
int byref(YN &) { return 1; }
template <class T> typename bad<T>::type byref(T &) { return 0; }
int cref(const YN &) { return 1; }
template <class T> typename bad<T>::type cref(const T &) { return 0; }
int rref(YN &&) { return 1; }
template <class T> typename bad<T>::type rref(T &&) { return 0; }
int arr(int *) { return 1; }
template <class T> typename bad<T>::type arr(T) { return 0; }
void fn() {}
int fptr(void (*)()) { return 1; }
template <class T> typename bad<T>::type fptr(T) { return 0; }
int dflt(YN, int = 0) { return 1; }
template <class T> typename bad<T>::type dflt(T) { return 0; }
int vararg(YN, ...) { return 1; }
template <class T> typename bad<T>::type vararg(T, ...) { return 0; }

struct S {
  int f(YN) { return 1; }
  template <class T> typename bad<T>::type f(T) { return 0; }
  int g(YN) & { return 2; }
  int g(YN) && { return 1; }
  template <class T> typename bad<T>::type g(T) { return 0; }
  int h(this S &, YN) { return 1; }
  template <class T> typename bad<T>::type h(this S &, T) { return 0; }
  int operator()(YN) { return 1; }
  template <class T> typename bad<T>::type operator()(T) { return 0; }
  int operator+(YN) { return 1; }
  template <class T> typename bad<T>::type operator+(T) { return 0; }
  int self() { YN v = YES; return f(v); }
};

int operator-(YN, YN) { return 1; }
template <class T> typename bad<T>::type operator-(T, T) { return 0; }

struct A {};
int operator<=>(const A &, const A &) { return 1; }
template <class T> typename bad<T>::type operator<(const T &, const T &) { return 0; }

struct C {
  int k;
  C(YN, int) : k(1) {}
  template <class T> C(T, typename bad<T>::type) : k(0) {}
};

namespace N {
struct R { int a[2] = {1, 0}; };
int *begin(R &r) { return r.a; }
int *end(R &r) { return r.a + 1; }
template <class T> typename bad<T>::type *begin(T &&);
template <class T> typename bad<T>::type *end(T &&);
}

template <class U> struct W {
  int pick(YN) requires (sizeof(U) == 4) { return 1; }
  int pick(YN) { return 2; }
  template <class T> typename bad<T>::type pick(T) { return 0; }
};

int main() {
  YN v = YES;
  const YN cv = YES;
  int a[3];
  S s;
  const A x;
  W<int> w;
  int sum = 0;
  for (int e : N::R()) sum += e;
  int ok = rank(v) + rank(cv) + byref(v) + cref(cv) + rref(YES) + arr(a) + fptr(fn) +
           dflt(v) + vararg(v) + s.f(v) + S().g(v) + s.h(v) + s(v) + (s + v) + s.self() +
           (v - v) + (x < x ? 0 : 1) + C(v, 1).k + C{v, 1}.k + sum + w.pick(v);
  return ok == 21 ? 0 : 1;
}
