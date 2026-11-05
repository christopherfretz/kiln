// [lex.nullptr], [conv.ptr]/1, [basic.fundamental]/15 (C++11, N2431):
// `nullptr` and std::nullptr_t; conversions and overload resolution.
// WP-646 feature probe.
//@ expect: run
//@ std: c++11
//@ strict-diags
//@ stdout: p i n 1
extern "C" int printf(const char*, ...);
typedef decltype(nullptr) nullptr_t;
void f(int*) { printf("p "); }
void f(int) { printf("i "); }
void g(nullptr_t) { printf("n "); }
struct S { int m; };
static_assert(sizeof(nullptr_t) == sizeof(void*), "size");
constexpr int S::*pm = nullptr;
static_assert(pm == nullptr && !nullptr, "member pointer and bool context");
int main() {
  f(nullptr);
  f(0);
  g(nullptr);
  void (*fp)() = nullptr;
  printf("%d\n", fp == nullptr && nullptr == (int*)0);
}
