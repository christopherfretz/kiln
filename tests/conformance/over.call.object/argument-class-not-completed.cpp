// [over.call.object]/1-3, [temp.inst]/2, [over.match.viable]/3: a call of
// an object of class type looks up the object's `operator()` and
// conversion functions only (no argument-dependent lookup), so it does
// not instantiate an argument's class template specialization; a
// candidate rejected by its constraints never converts the argument
// either. Instantiating `W<NC>` here would be ill-formed (libc++'s
// `sentinel_wrapper` of a non-copyable type in a niebloid concept check).
//@ expect: compile-ok
//@ strict-diags
template <class T> T&& declval();
struct NC { NC(const NC&) = delete; };
template <class I> I base(I i) { return i; }
template <class It> struct W { decltype(base(declval<It>())) base_; };

template <class T> concept Never = false;
struct Fn {
  template <class R, class O> requires Never<R> void operator()(R&&, O) const;
};
inline constexpr Fn fn{};

namespace q {
template <class T> struct rr { using type = T; };
template <class T> struct rr<T&> { using type = T; };
template <class T> T&& fwd(typename rr<T>::type& t) { return static_cast<T&&>(t); }
} // namespace q

template <class I, class S = W<I>>
concept Callable = requires(I&& i, S&& s) { fn(q::fwd<I>(i), q::fwd<S>(s)); };
static_assert(!Callable<NC>);
