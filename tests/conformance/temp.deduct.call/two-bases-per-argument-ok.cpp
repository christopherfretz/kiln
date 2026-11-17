// [temp.deduct.call]/4.3 and /5, the accepted side: one class reached by
// two paths (a virtual base, or the same base twice) is one possible
// deduced A; a class that is itself a specialization of the template is
// the deduced A and its bases are not considered; through a pointer; and a
// unique base that conflicts with another argument's deduction fails that
// candidate only. Found running GCC's C++ testsuite.
//@ expect: compile-ok
template <class T> struct B {};
struct V1 : virtual B<int> {};
struct V2 : virtual B<int> {};
struct DV : V1, V2 {};
template <class T> int f(const B<T>&);
template <class T> int g(const B<T>*);
void t(DV& dv) { f(dv); g(&dv); }

template <> struct B<long> : B<int>, B<char> {};
template <class T> constexpr int h(const B<T>&) { return sizeof(T); }
static_assert(h(B<long>{}) == sizeof(long));

template <class T> constexpr int k(T, const B<T>&) { return 1; }
constexpr int k(...) { return 2; }
struct E : B<int> {};
static_assert(k(1, E{}) == 1);
static_assert(k('c', E{}) == 2);

// Class template argument deduction through a base, by the same rule.
template <class T> struct W { W(const B<T>&); };
W w(E{});
static_assert(__is_same(decltype(w), W<int>));
