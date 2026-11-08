// Mangling differential: dependent expressions in `enable_if` and
// `decltype` (spec/abi.md §8.15, WP-818). An alias template written in
// an instantiation-dependent qualifier keeps its own name (`sr3ispIT_E`,
// not the aliased `is_ptr`), including an alias to a `decltype`; a
// reference to a non-dependent variable or function inside a dependent
// expression is `L <encoding> E`.

template <bool B, class T = void> struct enable_if {};
template <class T> struct enable_if<true, T> { typedef T type; };
template <class T> struct is_ptr { static const bool value = false; };
template <class T> struct is_ptr<T*> { static const bool value = true; };
template <class T, class U> struct both { static const bool value = true; };
template <class T> using isp = is_ptr<T>;
template <class T> using bothi = both<T, int>;
template <class... P> int helper(int);
template <class... P> using And = decltype(helper<P...>(0));
template <class T, typename enable_if<isp<T>::value, int>::type = 0> void f1(T) {}
template <class T, typename enable_if<bothi<T>::value, int>::type = 0> void f2(T) {}
template <class T, typename enable_if<sizeof(And<T>) == 4, int>::type = 0> void f3(T) {}
template <class U> struct Outer {
  template <class T, typename enable_if<both<isp<T>, U>::value, int>::type = 0> void m(T) {}
  template <class T, typename enable_if<isp<U>::value && isp<T>::value, int>::type = 0> void n(T) {}
};
void use() { int* p = nullptr; f1(p); f2(1); f3(1); Outer<int*>().m(p); Outer<int*>().n(p); }

constexpr int four = 4;
int fn();
template <class T> struct Holder { static constexpr int n = 4; };
template <class T, typename enable_if<sizeof(T) == four, int>::type = 0> void f4(T) {}
template <class T> auto f5(T t) -> decltype(t + fn()) { return t; }
template <class T, typename enable_if<sizeof(T) == Holder<int>::n, int>::type = 0> void f6(T) {}
void use2() { f4(1); f5(1); f6(1); }
