// WP-492, [basic.start.static]/2-3: a cast to or from a function pointer
// is not a core constant expression ([expr.const]/5.15), but a fold
// continues past it keeping the function, so the constant
// initialization fails and the re-fold (clang
// `ConstantEmitter::tryEmitPrivateForVarInit`) makes each of these a
// static initializer, not a dynamic one: a C-style cast of `&mk` or `mk`,
// a reinterpret_cast, an array and a struct of them (an initializer list
// is re-folded element-wise, as clang's `ConstExprEmitter`), and a `const`
// pointer cast back to its function's type, whose call `p5(1)` then folds
// to a direct call. A call through a pointer of another function type
// does not fold (`n2` is a dynamic initializer); one differing only in
// `noexcept` does (spec/oven.md "Pointers that point at nothing").
int mk(int);
constexpr int cmk(int v) { return v + 1; }
int x;
struct S { static int sm(); };
void (*p1)() = (void (*)())&mk;
void (*p2)() = (void (*)())mk;
auto p4 = reinterpret_cast<void (*)(long)>(&mk);
void (*arr[3])() = {(void (*)())&mk, (void (*)())&S::sm};
struct H { void (*f)(); int n; };
H h = {(void (*)())&mk, __builtin_is_constant_evaluated() ? x : 1};
const auto p5 = (int (*)(int))(void (*)())&mk;
int use() { return p5(1); }
int n1 = ((int (*)(int))(void (*)())&cmk)(1);
int n2 = ((int (*)(long))&cmk)(1);
int n3 = ((int (*)(int) noexcept)&cmk)(2);
bool same = (void (*)())&mk == (void (*)())mk;
