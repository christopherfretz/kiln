export module tid_mod;
export namespace tm {
template <class T> constexpr int v = 0;
template <class T> struct K {};
template <class T> int f(T) { return 0; }
template <class T> int g(T t) { return v<T> + v<K<T>> + f<T>(t); }
template <class T> struct S {
  template <class U> static constexpr int mw = 1;
  int a() { return this->template mw<T> + v<T>; }
};
} // namespace tm
