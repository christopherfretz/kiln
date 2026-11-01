// [temp.spec.partial.member]/1, [temp.mem]/1: a partial specialization of a member class template
// declared and defined out of line, `template<class T> template<class U> struct
// Outer<T>::Inner<U*>`, is matched when `Outer<X>::Inner<...>` is instantiated, and names in its
// body are found in `Outer<T>` ([class.nest]/3)
//@ expect: run
//@ strict-diags
namespace ns {
template <class T>
struct Outer {
  using type = T;
  static constexpr int base = 100;
  template <class U, class V = int>
  struct Inner {
    static constexpr int v = 0;
  };
  template <class U>
  struct Inner<U, U> {
    static constexpr int v = 5;
  };
};
}  // namespace ns
template <class T>
template <class U>
struct ns::Outer<T>::Inner<U*, int> {
  static constexpr int v = base + sizeof(type);
  type t{};
  int get() const { return v + int(sizeof(U)); }
};
template <class T>
template <class U>
struct ns::Outer<T>::Inner<U**, int> {
  static constexpr int v = 2;
};
namespace ns {
template <class T>
template <class U>
struct Outer<T>::Inner<U&, int> {
  static constexpr int v = 3;
};
}  // namespace ns
static_assert(ns::Outer<long>::Inner<char>::v == 0);
static_assert(ns::Outer<long>::Inner<char*>::v == 108);
static_assert(ns::Outer<int>::Inner<char**>::v == 2);
static_assert(ns::Outer<int>::Inner<int&>::v == 3);
static_assert(ns::Outer<int>::Inner<int, int>::v == 5);
int main() {
  ns::Outer<short>::Inner<double*> x;
  return x.get() == 110 ? 0 : 1;
}
