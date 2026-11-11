export module imgsl;
export template <int N> struct F { unsigned a : N; int b = N + 1; };
export template <class T> struct W { static const int w = sizeof(T); };
export inline int f(unsigned v) {
  F<3> x{};
  x.a = v;
  return int(x.a) + x.b;
}
export inline int h() { return W<int>::w; }
