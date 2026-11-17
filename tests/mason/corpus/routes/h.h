// one header through #include, a PCH and a header unit
#ifndef ROUTES_H
#define ROUTES_H
template <class T> struct Box {
  T v;
  T get() const { return v; }
};
inline int f(int a) {
  auto l = [a](int b) { return a + b; };
  Box<int> x{a};
  return l(x.get());
}
template <class T> inline T twice(T t) { return t + t; }
inline int g = twice(4);
#endif
