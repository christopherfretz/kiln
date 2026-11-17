// [temp.constr.normal]/1.4, [temp.constr.atomic]/3: `mc<X>` whose argument
// fails to substitute (`declval<R&>().begin()` for `R = int*`) is not
// satisfied, also when the concept's constraint names another concept-id
// with a pack parameter (`cf<T, T>` of `template<class T, class... A>
// concept cf`): that atom names the failed parameter, so it is not
// satisfied, and the candidate is dropped for the fallback. Reduced from
// real code (range-v3's `sortable<iterator_t<Rng>>` reaching
// `constructible_from<T, T>` for a non-range `Rng`).
//@ args: -std=c++20
template <class T> T&& declval();
template <class T, class... A> concept cf = __is_constructible(T, A...);
template <class T> concept mc = cf<T, T>;
template <class T> concept mc2 = mc<T> && cf<T>;

template <class R> requires mc<decltype(declval<R&>().begin())> int s(R&&);
long s(...);
template <class R> requires mc2<decltype(declval<R&>().begin())> int t(R&&);
long t(...);

struct V { int* begin(); };
int* p;
V v;
static_assert(sizeof(s(p)) == sizeof(long));
static_assert(sizeof(s(v)) == sizeof(int));
static_assert(sizeof(t(p)) == sizeof(long));
static_assert(sizeof(t(v)) == sizeof(int));
static_assert(!mc<void>);
