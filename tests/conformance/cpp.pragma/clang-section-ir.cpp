// [cpp.pragma]/1: #pragma clang section: variable definitions with static or thread storage
// duration and no section attribute get the current bss/data/rodata/relro
// sections as LLVM global attributes, function definitions (not lambdas or
// implicit functions) the text section unless a section attribute names one;
// '""' clears a kind, adjacent literals concatenate. WP-1744.
//@ expect: ir
//@ std: c++20
#pragma clang section bss="B" data="D" rodata="R" relro="L" text="T"
int g1;
int g2 = 3;
const int g3 = 4;
extern const int g4 = 5;
int *const g5 = &g1;
extern int g6;
int __attribute__((section("X"))) g7 = 1;
thread_local int g8 = 2;
struct S { static int sm; static inline int si = 3; void f() {} void h(); S(); };
int S::sm = 9;
void S::h() {}
S::S() = default;
template <class T> T tv = T(1);
template <class T> void tf() {}
int fn() { static int sl = 7; return sl + tv<int>; }
void fs() __attribute__((section("Y")));
void fs() {}
void use() { tf<int>(); S s; s.f(); auto l = []{ return 1; }; l(); }
#pragma clang section bss="" text=""
int g9;
void after() {}
#pragma clang section data="D2" "x"
int g10 = 1;
