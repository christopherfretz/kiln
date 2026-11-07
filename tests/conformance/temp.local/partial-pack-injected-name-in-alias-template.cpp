// [temp.local]/1, [temp.spec.partial.general]: the injected-class-name of
// a partial specialization whose arguments end in a pack expansion
// (`vd<seq<I...>, T...>`) names `vd<seq<I...>, T...>` with the arguments
// as written. Used in a member alias template, it is substituted with
// the alias's arguments and its arguments are checked against the
// primary's parameters again (Boost.Fusion `vector_data::store_at`).
//@ expect: run
//@ stdout: 7 a 2
extern "C" int printf(const char*, ...);

template <unsigned long... I> struct seq {};
template <unsigned long I, typename T> struct store { using elem_type = T; T elem; };
template <unsigned long J, typename U> store<J, U> store_at_impl(store<J, U>*);

template <typename I, typename... T> struct vd;
template <unsigned long... I, typename... T>
struct vd<seq<I...>, T...> : store<I, T>... {
  template <unsigned long J>
  using store_at = decltype(store_at_impl<J>(static_cast<vd*>(nullptr)));
  template <typename J> typename store_at<J::value>::elem_type& at(J) {
    return store_at<J::value>::elem;
  }
  template <class> using self = vd;
  static constexpr int size = sizeof...(T);
};
template <unsigned long N> struct int_ { static constexpr unsigned long value = N; };

int main() {
  vd<seq<0, 1>, int, char> v{};
  v.at(int_<0>{}) = 7;
  v.at(int_<1>{}) = 'a';
  printf("%d %c %d\n", v.at(int_<0>{}), v.at(int_<1>{}),
         vd<seq<0, 1>, int, char>::self<void>::size);
}
