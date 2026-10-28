//@ expect: ast
struct Pair { int a; long b; };
constexpr int i = 6 * 7;
constexpr bool yes = i > 40;
constexpr char ch = 'k';
constexpr unsigned long long big = ~0ULL;
constexpr int arr[4] = {1, 2};
constexpr Pair pr = {3, 4};
constexpr Pair prs[2] = {{1, 2}};
constexpr const int *np = nullptr;
constexpr int neg = -i;
