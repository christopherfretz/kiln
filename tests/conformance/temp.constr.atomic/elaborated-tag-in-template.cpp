// [temp.constr.atomic]/3, [basic.scope.pdecl]/7, [class.friend]/11,
// [temp.dep.type]/9: a class first declared by an elaborated-type-specifier
// inside a template (a parameter type, a template argument, a friend) is a
// member of the enclosing namespace, not of the template, so its type is not
// dependent: an atomic constraint naming it is a non-dependent constant after
// substitution, and overload resolution compares it like any class. This is
// libc++'s __bit_iterator::operator=(const _If<_IsConst, struct __private_nat,
// __bit_iterator>&), probed by assignable_from<vector<bool>::iterator&, ...>.
//@ expect: run
//@ stdout: 2 2 1 7
extern "C" int printf(const char*, ...);

template <bool> struct IfImpl;
template <> struct IfImpl<true> { template <class A, class B> using Select = A; };
template <> struct IfImpl<false> { template <class A, class B> using Select = B; };
template <bool C, class A, class B> using If = typename IfImpl<C>::template Select<A, B>;
template <class T> T&& fwd(T& t) { return static_cast<T&&>(t); }

template <class C, bool IsConst> struct bit_iterator {
  int v = 0;
  bit_iterator() {}
  bit_iterator(const bit_iterator<C, false>& o) : v(o.v) {}
  bit_iterator& operator=(const If<IsConst, struct private_nat, bit_iterator>& o) {
    v = o.v + 1;
    return *this;
  }
};

template <class L, class R>
concept assignable_from = requires(L l, R&& r) {
  { l = fwd<R>(r) };
};
static_assert(assignable_from<bit_iterator<int, false>&, bit_iterator<int, false>>);
static_assert(assignable_from<bit_iterator<int, true>&, bit_iterator<int, true>>);
static_assert(!assignable_from<bit_iterator<int, true>&, int>);

// A parameter, a friend and a namespace-scope template's parameter.
template <class T> struct A { void f(struct P*); friend struct F; };
template <class T> void g(struct Q*);
struct F;
struct S {};
int pick(const P&) { return 1; }
int pick(const S&) { return 2; }
int pick(const F&) { return 3; }
int pick(const Q&) { return 4; }
template <class T> int dep_pick(T t) { return pick(t); }

// A later definition completes the same, non-dependent class.
struct P { int x = 7; };
static_assert(requires(P p) { p.x; });

int main() {
  bit_iterator<int, false> a, b;
  a = b;
  a = b;
  bit_iterator<int, true> c;
  c = a;
  printf("%d %d %d %d\n", pick(S{}), dep_pick(S{}), c.v, P{}.x);
}
