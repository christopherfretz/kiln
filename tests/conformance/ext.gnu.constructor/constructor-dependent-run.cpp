// A value-dependent GNU `constructor(N)` / `destructor(N)` priority is
// evaluated when the function is instantiated (clang 22
// `instantiateDependentConstructorAttr`; WP-807); each explicit
// instantiation gets its own `@llvm.global_ctors` entry. clang 19
// rejects it (constructor-diags-clang19.cpp).
//@ requires: clang>=22
//@ expect: run
//@ stdout: tn 200 tn 300 main X 150
extern "C" int printf(const char*, ...);
template <int N> __attribute__((constructor(N))) void tn() { printf("tn %d ", N); }
template <int N> struct X {
  __attribute__((destructor(N))) static void f() { printf("X %d\n", N); }
};
template void tn<300>();
template void tn<200>();
template struct X<150>;
int main() { printf("main "); }
