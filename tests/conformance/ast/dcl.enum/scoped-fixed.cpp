//@ expect: ast
enum class Small : unsigned char { a, b = 3, c };
enum struct Wide : long long { big = 1LL << 40 };
enum Plain : short { p0 };
int to_int(Small s) { return (int)s; }
