// The written-reference table (spec/ast.md §18.9.1, WP-3695): the
// lookup-context (naming) class of Member and Qualified per-specialization
// rows, dumped as `naming=<class>` only when it differs from the entity's
// semantic parent class.
//@ written-refs: spec-naming-class.refs
//@ std: c++20
namespace n {
template <class T> struct S { static int s(); };
}
template <class B> struct Der : B {};
struct Der2 : n::S<int> {};

template <class T> using A = decltype(T::s());
template <class T> auto call() -> decltype(T::s());

struct LeafBase {
  int g() const;
  int v;
};
struct LeafDer : LeafBase {};

struct MidBase {
  int f() const;
  int f(int) const;
  int x;
  int operator+(int) const;
  LeafDer a;
  LeafDer b;
};
struct MidDer : MidBase {};

template <class T> int member_use(const T &t) {
  return t.f() + t.f(1) + t.x + (t + 1) + t.a.g() + t.a.v;
}

template <class T> struct Host : T {
  int run() const { return this->f() + this->x + this->b.g(); }
};

template <class T> concept HasS = requires(const T &t) { T::s(); t.f(); };
struct Both : n::S<int>, MidBase {};
static_assert(HasS<Both>);

A<Der<n::S<int>>> a1 = 0;
A<Der2> a2 = 0;
A<n::S<int>> a3 = 0;
int c1 = call<Der<n::S<int>>>();
int c2 = call<n::S<int>>();
int m1 = member_use(MidDer{});
int m2 = member_use(MidBase{});
int h1 = Host<MidDer>{}.run();
