// [temp.variadic]/5, [temp.deduct.call]/1: libstdc++'s `std::tuple_cat`
// (tuple:2129) without the library. `concater<R, index_seq<Is...>, T,
// Ts...>::run(T&& t, Ts&&... ts, Us&&... us)` peels the first tuple off
// the class's pack `Ts`, appends its elements (`get<Is>(t)...`) to the
// deduced pack `Us` and recurses on `Ts`; the parameters of `ts` and `us`
// must keep to their own packs at every step, including when a tuple is
// empty (`Is` and the next step's `Ts` empty). The real case is
// a downstream log-once helper: `tuple_cat(tuple<E>, tuple<int>,
// tuple<vector<int>>, tuple<>)`.
// Verified against clang 21.
//@ expect: run
//@ strict-diags
//@ stdout: 4 7 3 2 30 5 | 0 | 2 1 9
#include <stdio.h>

typedef decltype(sizeof 0) size_t;
template <class T> struct rm_ref { typedef T type; };
template <class T> struct rm_ref<T &> { typedef T type; };
template <class T> struct rm_ref<T &&> { typedef T type; };
template <class T> T &&fwd(typename rm_ref<T>::type &x) { return static_cast<T &&>(x); }

template <size_t... Is> struct index_seq {};
template <size_t N, size_t... Is> struct make_seq : make_seq<N - 1, N - 1, Is...> {};
template <size_t... Is> struct make_seq<0, Is...> { typedef index_seq<Is...> type; };

// A tuple as a chain of bases, as libstdc++'s _Tuple_impl.
template <size_t I, class... Ts> struct tuple_impl {};
template <size_t I, class H, class... R> struct tuple_impl<I, H, R...> : tuple_impl<I + 1, R...> {
  H head;
  tuple_impl() = default;
  template <class UH, class... UR>
  tuple_impl(UH &&h, UR &&...r) : tuple_impl<I + 1, R...>(fwd<UR>(r)...), head(fwd<UH>(h)) {}
};
template <class... Ts> struct tuple : tuple_impl<0, Ts...> {
  static const size_t size = sizeof...(Ts);
  tuple() = default;
  template <class... Us>
  explicit tuple(Us &&...us) : tuple_impl<0, Ts...>(fwd<Us>(us)...) {}
};
template <size_t I, class H, class... R> H &get_impl(tuple_impl<I, H, R...> &t) { return t.head; }
template <size_t I, class... Ts> auto &&get(tuple<Ts...> &&t) {
  return static_cast<decltype(get_impl<I>(t)) &&>(get_impl<I>(t));
}

template <class... Tuples> struct cat_result;
template <> struct cat_result<> { typedef tuple<> type; };
template <class... Ts> struct cat_result<tuple<Ts...>> { typedef tuple<Ts...> type; };
template <class... Ts, class... Us, class... Rest>
struct cat_result<tuple<Ts...>, tuple<Us...>, Rest...> : cat_result<tuple<Ts..., Us...>, Rest...> {};

template <class... Tuples> struct first_indices { typedef index_seq<> type; };
template <class T, class... Rest> struct first_indices<T, Rest...> {
  typedef typename make_seq<rm_ref<T>::type::size>::type type;
};

template <class Ret, class Seq, class... Tuples> struct concater;
template <class Ret, size_t... Is, class T, class... Ts> struct concater<Ret, index_seq<Is...>, T, Ts...> {
  template <class... Us> static Ret run(T &&t, Ts &&...ts, Us &&...us) {
    typedef typename first_indices<Ts...>::type next_seq;
    return concater<Ret, next_seq, Ts...>::run(fwd<Ts>(ts)..., fwd<Us>(us)...,
                                               get<Is>(fwd<T>(t))...);
  }
};
template <class Ret> struct concater<Ret, index_seq<>> {
  template <class... Us> static Ret run(Us &&...us) { return Ret(fwd<Us>(us)...); }
};

template <class... Tuples> typename cat_result<typename rm_ref<Tuples>::type...>::type tuple_cat(Tuples &&...ts) {
  typedef typename cat_result<typename rm_ref<Tuples>::type...>::type ret;
  typedef typename first_indices<Tuples...>::type seq;
  return concater<ret, seq, Tuples...>::run(fwd<Tuples>(ts)...);
}

struct E { int code; };
struct vec { int n; int data[3]; };

int main() {
  // tuple<E>, tuple<int>, tuple<vec>, tuple<> as the log-once helper; then a tuple of
  // two with an empty one first and in the middle.
  auto r = tuple_cat(tuple<E>(E{7}), tuple<int>(3), tuple<vec>(vec{2, {30, 0, 0}}), tuple<>(),
                     tuple<char>((char)5));
  printf("%d %d %d %d %d %d | ", (int)decltype(r)::size, r.head.code,
         static_cast<tuple_impl<1, int, vec, char> &>(r).head,
         static_cast<tuple_impl<2, vec, char> &>(r).head.n,
         static_cast<tuple_impl<2, vec, char> &>(r).head.data[0],
         static_cast<tuple_impl<3, char> &>(r).head);
  auto e = tuple_cat(tuple<>(), tuple<>());
  printf("%d | ", (int)decltype(e)::size);
  auto m = tuple_cat(tuple<>(), tuple<int, long>(1, 9L), tuple<>());
  printf("%d %d %d\n", (int)decltype(m)::size, m.head,
         (int)static_cast<tuple_impl<1, long> &>(m).head);
}
