// [expr.unary.noexcept]/1-3: the noexcept operator is a bool prvalue whose
// operand is unevaluated; it is false only when the operand is potentially
// throwing.
//@ expect: ast
int may();
void wont() noexcept;
int add(int a, int b) noexcept { return a + b; }
void (*pm)();
void (*pw)() noexcept;
constexpr bool a = noexcept(may());
constexpr bool b = noexcept(wont());
constexpr bool c = noexcept(1 + 2);
constexpr bool d = noexcept(add(1, 2) + add(3, 4));
constexpr bool e = noexcept(pm());
constexpr bool f = noexcept(pw());
constexpr bool g = noexcept(sizeof(may()));
constexpr bool h = noexcept(noexcept(may()));
int use() { return noexcept(may()) ? 1 : 0; }
