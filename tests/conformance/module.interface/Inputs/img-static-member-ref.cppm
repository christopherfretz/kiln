export module imgsm;
export template <int N> struct B { static constexpr int v = N; };
export template <class T> struct W { static const int w = sizeof(T); };
export inline int g() { return B<2>::v; }
export inline int h() { return W<int>::w + B<3>::v * 10; }
export inline const int *p() { return &B<7>::v; }
