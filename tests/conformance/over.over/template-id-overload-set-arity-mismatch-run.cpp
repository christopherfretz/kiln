// [over.over]/3, [temp.deduct.funcaddr]: for `&wrap<1>` with target type
// `void (*)(void*, int)`, each template in the overload set gets the
// explicit argument and is deduced against the target; the one-parameter
// template does not match and is dropped, so the two-parameter one is
// selected with `T0 = int`. The Boost.FunctionTypes `interface` example
// (`&member<...>::wrap<&C::f>`) hits this.
//@ expect: run
//@ xfail(kiln): WP-787 follow-up, a template-id naming a set of function templates is not matched against the target type template by template
//@ stdout: 1 5
extern "C" int printf(const char*, ...);
template<int N> void wrap(void*) { printf("0\n"); }
template<int N, class T0> void wrap(void*, T0 a0) { printf("%d %d\n", N, a0); }
void (*p)(void*, int) = &wrap<1>;

int main() { p(nullptr, 5); }
