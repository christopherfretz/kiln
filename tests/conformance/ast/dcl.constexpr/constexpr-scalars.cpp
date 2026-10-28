//@ expect: ast
constexpr int k = 4;
constexpr long big = k * 1000000000L;
constexpr double half = 0.5;
constexpr bool on = k > 3 && half < 1.0;
constexpr char letter = 'a' + k;
const int ci = k + 1;
int arr[k + ci];
