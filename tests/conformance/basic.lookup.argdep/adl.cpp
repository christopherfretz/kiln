// [basic.lookup.argdep]: argument-dependent lookup adds the functions of
// the associated namespaces and the hidden friends of the associated
// classes; a block-scope function declaration or a non-function
// suppresses it (/1, /3).
//@ expect: compile-ok
namespace N {
struct A {
  friend char hf(A);
  friend long hf(A, int) { return 0; }
};
char f(A);
struct B : A {};
enum E { e0 };
short g(E);
template <class T> struct W {};
int w(W<A> *);
} // namespace N
long f(int);
static_assert(__is_same(decltype(f(N::A())), char));
static_assert(__is_same(decltype(f(1)), long));
static_assert(__is_same(decltype(hf(N::A())), char));
static_assert(__is_same(decltype(hf(N::A(), 1)), long));
static_assert(__is_same(decltype(f(N::B())), char));
static_assert(__is_same(decltype(g(N::e0)), short));
static_assert(__is_same(decltype(w((N::W<N::A> *)0)), int));
N::A *pa();
char p(N::A *);
static_assert(__is_same(decltype(f(*pa())), char));
namespace M {
long f(N::A, int);
static_assert(__is_same(decltype(f(N::A(), 1)), long));
} // namespace M
void t() {
  long f(N::A);
  static_assert(__is_same(decltype(f(N::A())), long));
}
