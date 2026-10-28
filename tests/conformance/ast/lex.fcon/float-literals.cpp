//@ expect: ast
constexpr double d = 1.5;
constexpr float f = 0.25f;
constexpr double tiny = 1e-300;
constexpr double neg = -2.0;
constexpr double zero = 0.0;
constexpr float arr[3] = {1.0f, 2.5f};
double sum(float x) { return x + d * 3; }
