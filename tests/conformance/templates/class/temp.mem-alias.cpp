// [temp.mem]/1, [temp.alias]: member alias templates of a class template,
// including one taking a template template parameter.
template <class T> struct A {
  template <class U> using ptr = T *;
  template <class U> using pair = U[sizeof(T)];
  template <template <class> class TT> using wrap = TT<T>;
};
template <class X> struct Box { X x; };
A<int>::ptr<long> p = nullptr;
A<long>::pair<char> arr = {};
A<short>::wrap<Box> w = {7};
static_assert(sizeof(arr) == 8);
static_assert(sizeof(w) == 2);
static_assert(sizeof(*p) == 4);
