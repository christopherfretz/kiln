// [dcl.spec.auto.general]/3, [dcl.type.auto.deduct]/2-4, [dcl.fct]/22: a
// placeholder may sit in the return type of a function declarator nested
// under pointer and reference declarators and further trailing return
// types. A variable deduces it from its initializer (the function types
// must otherwise match), a function's return type from its return
// statement, and in the clause of a function declaration or a lambda it
// is an invented template parameter deduced from the call.
//@ expect: run
//@ stdout: 3 7 10 4 6 22 4
extern "C" int printf(const char*, ...);
template <class T> concept C = sizeof(T) == 4;
int inc(int x) { return x + 1; }
long twice(int x) { return 2L * x; }
int (*get(int))(int) { return inc; }
long (*getl(int))(int) { return twice; }
auto (*p)(int) -> auto = inc;
auto (&r)(int) -> auto = inc;
auto (*pp)(int) -> auto (*)(int) -> auto = get;
auto (*pc)(int) -> auto (*)(int) -> C auto = get;
auto (*pl)(int) -> auto (*)(int) -> long = getl;
static_assert(__is_same(decltype(p), int (*)(int)));
static_assert(__is_same(decltype(r), int (&)(int)));
static_assert(__is_same(decltype(pp), int (*(*)(int))(int)));
static_assert(__is_same(decltype(pl), long (*(*)(int))(int)));
auto ret(int) -> auto (*)(int) -> auto { return inc; }
static_assert(__is_same(decltype(ret(0)), int (*)(int)));
int call(auto (*cb)(int) -> auto (*)(int) -> auto) { return (int)cb(0)(5); }
int callc(auto (*cb)(int) -> auto (*)(int) -> C auto) { return cb(0)(2); }
template <class T> struct W {
  static constexpr auto (*q)(int) -> auto = inc;
};
int main() {
  auto l = [](auto (*cb)(int) -> auto (*)(int) -> auto) { return cb(0)(11); };
  printf("%d %d %d %d %d %d %d\n", p(2), r(6), pp(0)(9), (int)pl(0)(2),
         call(get), (int)l(getl), W<int>::q(callc(get)));
}
