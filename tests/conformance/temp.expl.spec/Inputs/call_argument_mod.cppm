// The module of explicit-spec-call-argument-module-run.cpp.
export module call_argument_mod;
export constexpr int f(int x) { return x; }
int gx = 4;
export constexpr const int *g() { return &gx; }
export template <class T, int N> struct P { static constexpr int k = 0; };
template <> struct P<int, f(2)> { static constexpr int k = 2; };
export template <const int *p> struct Q { static constexpr int k = 0; };
template <> struct Q<g()> { static constexpr int k = 5; };
export template <bool B> struct BT { static constexpr int k = 0; };
template <> struct BT<f(1) == 1> { static constexpr int k = 1; };
