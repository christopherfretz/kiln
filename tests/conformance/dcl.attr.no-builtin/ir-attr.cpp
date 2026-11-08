// [dcl.attr.grammar]/5 The clang `no_builtin` attribute: a definition
// carries `"no-builtins"` (no names, the wildcard) or
// `"no-builtin-<name>"` per name, as does every call of it; inside it a
// named library function (any, for the wildcard) is called, not lowered
// as its builtin, while `__builtin_*` and the unnamed library builtins
// still lower. spec/codegen.md "no-builtin", WP-808.
//@ expect: ir
//@ args: --target=x86_64-unknown-linux-gnu -ftrivial-auto-var-init=uninitialized
extern "C" void* memset(void*, int, unsigned long);
extern "C" void* memcpy(void*, const void*, unsigned long);
extern "C" double ceil(double);
extern "C" void ext(char*);
// Outside a no_builtin function the library builtins are lowered.
void* lowered(char* p, const char* q) {
  memcpy(p, q, 4);
  ext(p);
  return memset(p, 0, 8);
}
double rounded(double x) { return ceil(x); }
__attribute__((no_builtin)) void all(char* p, double x) {
  memset(p, 0, 8);
  memcpy(p, p + 8, 2);
  ext(p);
  p[0] = (char)ceil(x);
}
__attribute__((no_builtin("memcpy", "memset"))) void named(char* p, double x) {
  memset(p, 0, 8);
  memcpy(p, p + 8, 2);
  p[0] = (char)ceil(x);
}
[[clang::no_builtin("memset")]] void one(char* p) {
  memset(p, 0, 8);
  memcpy(p, p + 8, 2);
  __builtin_memset(p, 1, 4);
}
// Two attributes compose; a repeated name counts once.
[[clang::no_builtin("memset", "memset")]] [[clang::no_builtin("memmove")]] void two() {}
void call_all(char* p) {
  all(p, 1.5);
  named(p, 2.5);
  one(p);
  two();
  void (*fp)(char*) = one;
  fp(p);
}
struct A {
  __attribute__((no_builtin("memset"))) void m(char* p) { memset(p, 0, 2); }
};
void call_m(A a, char* p) { a.m(p); }
// The attribute's `std-move` does not stop `std::move` folding.
namespace std {
template <class T> struct remove_reference { using type = T; };
template <class T> struct remove_reference<T&> { using type = T; };
template <class T> constexpr typename remove_reference<T>::type&& move(T&& t) noexcept {
  return static_cast<typename remove_reference<T>::type&&>(t);
}
}
[[clang::no_builtin("std-move")]] int named_move(int& x) {
  int&& r = std::move(x);
  return r;
}
// A thunk's call of its target carries no attribute part.
struct V { virtual ~V(); };
struct W { virtual void w(); };
struct X : V, W { void w() override; };
[[clang::no_builtin("memset")]] void X::w() {}
void thrower();
void caller() noexcept { thrower(); }
