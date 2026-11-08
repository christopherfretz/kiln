// [temp.explicit]/10, [temp.spec.partial.member]: an explicit instantiation
// definition of a class template defines its non-template members only.
// The partial specialization of a member class template it declares is a
// template, so its static data members (one with a dependent initializer)
// are not emitted; and a later implicit specialization of the member
// template (`hd<double>::is_it<char, int>`) is governed by its own kind,
// so its odr-used members are `linkonce_odr` and an unused one is not
// emitted (clang `InstantiateClassMembers` skips member templates). WP-812
// (Boost.Math's `hyperexponential_distribution::is_iterator`).
template <class T> struct tr { using d = int; };
template <class A, class B> struct same { static constexpr bool value = false; };
template <class A> struct same<A, A> { static constexpr bool value = true; };
template <class R> struct hd {
  template <typename T, typename = int> struct is_it { static constexpr bool value = false; };
  template <typename T> struct is_it<T, typename tr<T>::d> {
    static constexpr bool value = !same<typename tr<T>::d, void>::value;
    static constexpr int size = sizeof(T) + sizeof(R);
    static int count;
    int get() const { return size; }
  };
  template <typename T> struct plain { static constexpr int n = sizeof(T); };
  int f() const { return is_it<R>::value; }
};
template <class R> template <typename T> int hd<R>::is_it<T, typename tr<T>::d>::count = 3;
template struct hd<double>;

int use() {
  const bool *p = &hd<double>::is_it<char>::value;
  const int *q = &hd<double>::is_it<char, int>::size;
  const int *n = &hd<double>::plain<short>::n;
  hd<double>::is_it<long> x;
  return *p + *q + *n + hd<double>::is_it<char, long>::value + x.get() +
         hd<double>::is_it<char>::count;
}
