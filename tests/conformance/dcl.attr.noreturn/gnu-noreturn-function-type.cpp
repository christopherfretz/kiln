// GNU `noreturn` is part of the function type (clang's function type
// attribute): it applies to function pointers, references, lambdas and
// declarations, is merged across redeclarations and explicit
// specializations, may be dropped by a function pointer conversion, and a
// call through such a type ends control flow (-Wreturn-type,
// -Winvalid-noreturn). Reduced from glog's InstallFailureFunction.
//@ expect: compile-ok
//@ strict-diags
//@ diag: 53:43 warning "function declared 'noreturn' should not return"
template <class T, class U> struct same { static const bool value = false; };
template <class T> struct same<T, T> { static const bool value = true; };
typedef void (*fail_fn)() __attribute__((noreturn));
void install(fail_fn);
__attribute__((noreturn)) static void dump_and_exit() { throw 1; }
[[gnu::noreturn]] void g1();
void g2();
__attribute__((noreturn)) void g2();
__attribute__((noreturn)) void g3();
void g3();
void use() {
  install(&dump_and_exit);
  install(dump_and_exit);
  install(&g1);
  install(&g2);
  install(&g3);
  install([]() __attribute__((noreturn)) { throw 2; });
}
static_assert(!same<fail_fn, void (*)()>::value, "");
static_assert(same<decltype(&g1), fail_fn>::value, "");
void (*plain)() = &g1;
void (&plain_ref)() = g1;
auto either = true ? fail_fn(&g1) : plain;
static_assert(same<decltype(either), void (*)()>::value, "");
int pick(void (*)());
char pick(fail_fn);
static_assert(sizeof(pick(fail_fn(&g1))) == 1, "");
static_assert(sizeof(pick(plain)) == sizeof(int), "");
struct S {
  __attribute__((noreturn)) static void sm();
  __attribute__((noreturn)) void m();
};
fail_fn from_static = &S::sm;
void (S::*mp)() = &S::m;
template <class T> __attribute__((noreturn)) void t(T) { throw 3; }
template <> __attribute__((noreturn)) void t<int>(int) { throw 4; }
template void t<char>(char);
int call_ptr(fail_fn f) { f(); }
int call_ref(void (&f)() __attribute__((noreturn))) { f(); }
__attribute__((noreturn)) void forward(fail_fn f) { f(); }
fail_fn g_fail = reinterpret_cast<fail_fn>(&g2);
struct LogMessage { __attribute__((noreturn)) void Fail(); };
void LogMessage::Fail() { g_fail(); }
__attribute__((noreturn)) void returns() {}
