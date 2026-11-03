// [basic.lookup.argdep]/3: a class's associated entities are the class,
// the class it is a member of and its direct and indirect bases; only the
// class's own template arguments (when it is a specialization) add more.
// A base's template arguments, or those of the enclosing class of a
// nested class, do not (libc++'s view iterators derive from
// `__filter_iterator_category<View>` and must not find functions in the
// view's namespace).
//@ expect: compile-ok
//@ strict-diags
namespace adl {
struct A {};
void adl_func(const auto&);
} // namespace adl
namespace lib {
template <class T> struct B {};
struct E : B<adl::A> {};
template <class T> struct F : B<T> {};
template <class T> struct V {
  struct It : B<T> {};
  struct Plain {};
  template <class U> struct TIt {};
};
} // namespace lib
template <class X> concept Can = requires(X x) { adl_func(x); };
static_assert(!Can<lib::E>);
static_assert(!Can<lib::V<adl::A>::It>);
static_assert(!Can<lib::V<adl::A>::Plain>);
static_assert(!Can<lib::V<adl::A>::TIt<int>>);
static_assert(Can<lib::F<adl::A>>);
static_assert(Can<lib::V<adl::A>::TIt<adl::A>>);
static_assert(Can<lib::V<adl::A>*>);
struct G : adl::A {};
static_assert(Can<G>);
