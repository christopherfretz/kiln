// [temp.explicit]/10, [temp.spec.partial.member]: an explicit instantiation
// definition of a class template defines its non-template members; a
// partial specialization of a member class template is a template, so its
// static data members (here with a dependent initializer) are not emitted
// and stay usable through later specializations. Boost.Math's
// `hyperexponential_distribution::is_iterator`; kiln reported "expression
// DependentScopeDeclRefExpr is not supported yet" (WP-812).
//@ expect: run
//@ std: c++20
//@ stdout: 0 1 1 8
extern "C" int printf(const char*, ...);
template <class T> struct tr { using d = int; };
template <class A, class B> struct same { static constexpr bool value = false; };
template <class A> struct same<A, A> { static constexpr bool value = true; };
template <class R> struct hd {
  template <typename T, typename = int> struct is_it { static constexpr bool value = false; };
  template <typename T> struct is_it<T, typename tr<T>::d> {
    static constexpr bool value = !same<typename tr<T>::d, void>::value;
    static constexpr int size = sizeof(T) + sizeof(R);
  };
  static constexpr bool f() { return is_it<R>::value; }
};
template struct hd<double>;
int main() {
  const bool* p = &hd<double>::is_it<char>::value;
  const int* q = &hd<double>::is_it<char, int>::size;
  printf("%d %d %d %d\n", hd<double>::is_it<char, long>::value, *p, hd<double>::f(), *q - 1);
}
