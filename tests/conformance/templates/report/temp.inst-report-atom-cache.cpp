// The §8.3 atom_cache (spec/sema-templates.md): an atomic constraint of a
// shared sub-concept is evaluated once per canonical argument list across
// every concept and constrained template reaching it; the totals row
// counts satisfaction checks and atom evaluations with their cache hits.
//@ inst-report: temp.inst-report-atom-cache.json
template <class T, class U> concept SameAs = __is_same(T, U);
template <class T> concept IsInt = SameAs<T, int>;
template <class T> concept IsIntToo = SameAs<T, int> && sizeof(T) == 4;
template <class T> concept Small = sizeof(T) <= 8;

template <class T> requires IsInt<T> && Small<T> int f(T) { return 1; }
template <class T> requires IsIntToo<T> int g(T) { return 2; }
template <class T> requires Small<T> int h(T) { return 3; }

int a = f(1);
int b = g(1);
int c = h(1);
int d = h(2);
static_assert(IsInt<int> && IsIntToo<int> && !IsInt<long>);
static_assert(SameAs<int, int>);

