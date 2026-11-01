//@ expect: compile-fail
//@ strict-diags
//@ diag: 16:1 error "decomposition declaration cannot be declared 'constexpr'"
//@ diag: 17:1 error "decomposition declaration cannot be declared 'constinit'"
//@ diag: 18:1 error "decomposition declaration cannot be declared 'inline'"
//@ diag: 19:1 error "decomposition declaration cannot be declared with 'constexpr inline' specifiers"
//@ diag: 23:1 error "decomposition declaration cannot be declared 'typedef'"
//@ diag: 25:3 error "decomposition declaration cannot be declared 'extern'"
//@ diag: 25:15 error "declaration of block scope identifier with linkage cannot have an initializer"
// [dcl.pre]/6: the decl-specifier-seq of a structured binding declaration
// may contain only static, thread_local, auto and cv-qualifiers; clang
// names the first offending specifier, or all of them. An extern block
// scope declaration also cannot have an initializer.
struct S { int a, b; };
constexpr S cs{1, 2};
constexpr auto [a1, b1] = cs;
constinit auto [a2, b2] = cs;
inline auto [a3, b3] = cs;
constexpr inline auto [a4, b4] = cs;
static auto [a5, b5] = cs;
thread_local auto [a6, b6] = cs;
static thread_local auto [a7, b7] = cs;
typedef auto [ta, tb] = cs;
void f() {
  extern auto [e1, e2] = cs;
  static auto [s1, s2] = cs;
  thread_local auto [t1, t2] = cs;
}
