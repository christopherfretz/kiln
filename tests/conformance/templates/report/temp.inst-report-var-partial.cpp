// [temp.inst]/7, [temp.spec.partial.match]/2, [temp.mem]: variable template
// specializations, specializations instantiated from a partial
// specialization, and member templates in kiln's
// -fkiln-instantiation-report=json (spec/sema-templates.md §12).
//@ inst-report: temp.inst-report-var-partial.json
template <class T> constexpr int rank_v = 0;
template <class T> constexpr int rank_v<T *> = 1 + rank_v<T>;
template <class T> struct P { T v; };
template <class T> struct P<T *> { T *p; int n; };
template <class T> struct A {
  template <class U> struct B { T t; U u; };
};
static_assert(rank_v<int ***> == 3);
static_assert(rank_v<char *> == 1);
static_assert(sizeof(P<int *>) == 16);
static_assert(sizeof(P<char *>) == 16);
static_assert(sizeof(P<char>) == 1);
static_assert(sizeof(A<int>::B<char>) == 8);
