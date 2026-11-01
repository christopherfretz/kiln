// [temp.inst]/3, [class.base.init]/9.1: the default member initializer of
// a member of a class template specialization is instantiated only when
// a constructor or an aggregate initialization uses it (clang
// `InstantiateInClassInitializer`), so an initializer that would be
// ill-formed for the arguments is no error while every constructor
// initializes the member itself (libc++ `split_view::__base_ = _View()`
// with `_View = ref_view<int[N]>`). WP-223.
//@ expect: run
//@ stdout: 7 1 2 9 3 4
//@ exit: 0
extern "C" int printf(const char*, ...);

struct NoDef {
  int v;
  constexpr NoDef(int x) : v(x) {}
};
template <class T> concept defaultable = requires { T(); };

// The split_view shape: a constrained defaulted default constructor.
template <class T> struct View {
  T base = T();
  int n = 1;
  View() requires defaultable<T> = default;
  constexpr View(T b) : base(b) {}
};

// An aggregate: the initializer of an explicitly initialized member is
// not used; that of the others is.
template <class T> struct Agg {
  T t = T();
  int k = T(2).v;
};

// A member class of a class template.
template <class T> struct Outer {
  struct In {
    T t = T();
    int m = 3;
    constexpr In(int x) : t(x) {}
  };
};

// A default constructor that is never used: its exception specification
// is not needed either.
template <class T> struct Plain {
  T t = T();
  int m = 4;
  constexpr Plain(T x) : t(x) {}
};

static_assert(!defaultable<NoDef>);
static_assert(!__is_constructible(View<NoDef>));
static_assert(__is_constructible(View<int>));
constexpr View<NoDef> cv(NoDef(5));
static_assert(cv.base.v == 5 && cv.n == 1);

int main() {
  View<NoDef> v(NoDef(7));
  Agg<NoDef> a{NoDef(9)};
  Outer<NoDef>::In in(3);
  Plain<NoDef> p(NoDef(0));
  printf("%d %d %d %d %d %d\n", v.base.v, v.n, a.k, a.t.v, in.m, p.m);
}
