// [except.spec]/1-2: noexcept, noexcept(constant-expression) and throw()
// exception specifications on function declarators; the type keeps the
// spelling and the folded value.
//@ expect: ast
void f0() noexcept;
void f1() noexcept(true);
void f2() noexcept(false);
void f3() noexcept(sizeof(int) == 4);
void f4() throw();
constexpr bool yes = true;
void f5() noexcept(yes);
void f6() noexcept(1);
void (*fp)() noexcept = f0;
void f0() noexcept {}
int g(int x) noexcept(noexcept(f0())) { return x; }
