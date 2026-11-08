// GNU `returns_twice` (WP-807): the function's declaration, definition
// and direct calls get `returns_twice`, as do the C library functions
// clang knows return twice (`setjmp`, ...); an indirect call does not.
typedef long jmp_buf[48];
extern "C" int setjmp(jmp_buf);
extern "C" int _setjmp(jmp_buf);
int my_rt() __attribute__((returns_twice));
[[gnu::returns_twice]] int my_rt2() { return 0; }
struct S { __attribute__((returns_twice)) int m(); };
int (*fp)();
jmp_buf jb;
int use(S &s) { return setjmp(jb) + _setjmp(jb) + my_rt() + my_rt2() + s.m() + fp(); }
