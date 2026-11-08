// [dcl.ambig.res]/1, [temp.res.general]/3: `has_key<F, int>::type` names a
// type: the base `impl<int>::apply<F, int>` derives from
// `not_<F::type>::type`, the typedef `type` that `not_` inherits from
// `bool_<!C>` through `not_impl<C>`, which names `bool_` itself. So the
// parenthesized `(int (*)(has_key<F, int>::type))g` is a cast.
// Boost.Fusion `filter_view.cpp:117`, `joint_view.cpp:162` and
// `iterator_range.cpp:98` (`has_key_impl<Tag>::apply`) hit this.
//@ expect: run
//@ stdout: 6
extern "C" int printf(const char*, ...);
template<bool C> struct bool_ { static const bool value = C; typedef bool_ type; };
template<bool C> struct not_impl : bool_<!C> {};
template<class T> struct not_ : not_impl<T::type::value> {};
template<class T> struct tag_of { typedef int type; };
template<class Tag> struct impl {
  template<class S, class K> struct apply : not_<typename S::type>::type {};
};
template<class S, class K>
struct has_key : impl<typename tag_of<S>::type>::template apply<S, K> {};
struct F { typedef bool_<false> type; };
int g(bool_<true>) { return 6; }
int main() {
  int r = ((int (*)(has_key<F, int>::type))g)(bool_<true>());
  printf("%d\n", r);
}
