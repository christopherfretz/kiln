// The written-reference table (spec/ast.md §18.4, WP-2471), role
// `designator`: every member name of an offsetof member-designator
// ([support.types.layout]/1) is an entry, through the `offsetof` macro
// too: `a.b[1].c` is three entries, each naming the member lookup found
// (the anonymous union's member itself, [class.union.anon]/1). In a
// template pattern whose designator waits for instantiation the names
// into a type that is not dependent are bound at the definition
// ([temp.res.general]/1, a value-dependent index notwithstanding); from
// the first dependent type on they are `dep`.
//@ written-refs: offsetof.refs
//@ std: c++20
#define offsetof(t, m) __builtin_offsetof(t, m)
struct In {
  int c;
  int d[3];
};
struct M {
  int x;
  In b[2];
};
struct S {
  int q;
  M a;
  union {
    int u;
    float f;
  };
};
constexpr unsigned long o1 = offsetof(S, a.b[1].c);
constexpr unsigned long o2 = __builtin_offsetof(S, u);
template <int N> constexpr unsigned long o3 = offsetof(S, a.b[N].d[1]);
template <class T> struct Dep {
  T t;
  int i;
};
template <class T> constexpr unsigned long o4 = offsetof(Dep<T>, t.c);
template <class T> constexpr unsigned long o5 = offsetof(T, a.b);
static_assert(o3<1> > o1 && o4<In> == 0 && o5<S> == offsetof(S, a.b));
