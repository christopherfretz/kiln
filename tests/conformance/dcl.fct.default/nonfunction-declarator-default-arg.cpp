// [dcl.fct.default]/3: a default argument is specified only in the
// parameter-declaration-clause of a function declaration (or a lambda
// declarator), not in a pointer-to-function, function reference, typedef,
// alias, type-id, nested parameter or return type; reported at the
// parameter's name, or at the `=` when it is unnamed. Valid pair:
// nonfunction-declarator-default-arg-valid.cpp (WP-1015).
//@ std: c++20
//@ expect: compile-fail
//@ diag: 25:20 error "default arguments can only be specified for parameters in a function declaration"
//@ diag: 26:20 error "default arguments can only be specified for parameters in a function declaration"
//@ diag: 27:17 error "default arguments can only be specified for parameters in a function declaration"
//@ diag: 28:29 error "default arguments can only be specified for parameters in a function declaration"
//@ diag: 30:17 error "default arguments can only be specified for parameters in a function declaration"
//@ diag: 31:25 error "default arguments can only be specified for parameters in a function declaration"
//@ diag: 32:22 error "default arguments can only be specified for parameters in a function declaration"
//@ diag: 34:29 error "default arguments can only be specified for parameters in a function declaration"
//@ diag: 35:25 error "default arguments can only be specified for parameters in a function declaration"
//@ diag: 36:33 error "default arguments can only be specified for parameters in a function declaration"
//@ diag: 37:20 error "default arguments can only be specified for parameters in a function declaration"
//@ diag: 38:27 error "default arguments can only be specified for parameters in a function declaration"
//@ diag: 39:27 error "default arguments can only be specified for parameters in a function declaration"
//@ diag: 40:27 error "default arguments can only be specified for parameters in a function declaration"
//@ diag: 42:21 error "default arguments can only be specified for parameters in a function declaration"
//@ diag: 43:23 error "default arguments can only be specified for parameters in a function declaration"
typedef void F(int = 1);
using U = void(int = 2);
void (*f())(int = 3);
int x = sizeof(void (*)(int y = 4));
struct S {
  void (*m)(int y = 5);
  static void (*sm)(int = 6);
  typedef void G(int = 7);
};
void h(int (*q)(int (*)(int = 8)));
template <void (*P)(int = 9)> struct T {};
void k(int z = 1, void (*w)(int a = 2) = nullptr);
void (*arr[2])(int = 3);
auto tr() -> void (*)(int = 4);
void (*pf(int a = 5))(int b = 6);
auto l = [](void (*p)(int = 7)) {};
void b() {
  if (void (*c)(int = 8) = nullptr) {}
  for (void (*fp)(int = 9) = nullptr;;) {}
}
