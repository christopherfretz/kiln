// _Float16 ([basic.fundamental], std::float16_t): an IEEE half 'half' in IR.
// clang 19: x86-64 evaluates arithmetic in float (excess precision, fpext /
// fptrunc around the operation); AArch64 operates on half directly.
_Float16 add(_Float16 a, _Float16 b) { return a + b; }
_Float16 mul(_Float16 a, float b) { return a * b; }
double cv(_Float16 x) { return x; }
int toi(_Float16 x) { return x; }
_Float16 fromi(int i) { return i; }
bool lt(_Float16 a, _Float16 b) { return a < b; }
_Float16 g = 1.5f16;
_Float16 neg(_Float16 a) { return -a; }
void inc(_Float16 &a) { a += 1; ++a; }
_Float16 chain(_Float16 x, _Float16 y, _Float16 z) { return x * y - z / x + y; }
_Float16 pos(_Float16 x) { return +x; }
_Float16 negsum(_Float16 x, _Float16 y) { return -(x + y); }
float wide(_Float16 x, _Float16 y) { return x + y; }
_Float16 cond(bool c, _Float16 x, _Float16 y) { return (c ? x : y) + x; }
void cassign(_Float16 &a, _Float16 b, double d) { a *= b + b; a -= d; a /= 2; }
_Float16 g2 = 0.1f16 * 3;
