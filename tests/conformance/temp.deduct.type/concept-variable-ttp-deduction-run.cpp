// [temp.deduct.type]/8-9 (P2841R7): a concept template parameter or a
// variable template template parameter is deduced from a template-argument
// of a simple-template-id `X<C>` / `Q<V>` in P, packs included; the deduced
// variable template is then read in constant evaluation and at run time
// through `V<T>` ([temp.names], spec/sema-templates.md §2.1.1). clang 22
// agrees. WP-746.
//@ expect: run
//@ strict-diags
//@ requires: clang>=22
template <class T> concept Int = __is_same(T, int);
template <class T, class U> concept Same = __is_same(T, U);
template <class T> concept Any = true;
template <class T> constexpr int sz = sizeof(T) == 1 ? 1 : 9;
template <class T, class U = int> constexpr int sz2 = sz<T> + sz<U>;

template <template <class> auto V> struct Q {};
template <template <class> auto V> constexpr int pick(Q<V>) { return V<char>; }
static_assert(pick(Q<sz>{}) == 1);
static_assert(pick(Q<sz2>{}) == 10);

template <template <class> concept C> struct QC {};
template <template <class> concept C> constexpr bool pickc(QC<C>) { return C<int>; }
static_assert(pickc(QC<Int>{}));

template <template <class> concept... Cs> struct QP {};
template <template <class> concept... Cs, class T>
constexpr bool every(QP<Cs...>, T) { return (Cs<T> && ...); }
static_assert(every(QP<Int, Any>{}, 1));
static_assert(!every(QP<Int, Any>{}, 1L));

template <template <class> auto... Vs> struct QV {};
template <template <class> auto... Vs> constexpr int total(QV<Vs...>) {
  return (0 + ... + Vs<char>);
}
static_assert(total(QV<sz, sz2>{}) == 11);

// Deduced from a nested template-id and from a pointer.
template <class> struct Box {};
template <template <class> auto V, class T> constexpr int inner(Box<Q<V>> *, T) { return V<T>; }
static_assert(inner(static_cast<Box<Q<sz>> *>(nullptr), 'c') == 1);

int run(int x) {
  int r = pick(Q<sz>{}) + total(QV<sz, sz2>{}) + pickc(QC<Int>{});
  return r + x;
}
int main() { return run(0) == 13 ? 0 : 1; }
