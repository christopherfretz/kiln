//@ expect: compile-fail
//@ strict-diags
//@ diag: 15:1 error "declaration cannot be declared 'inline'"
//@ diag: 19:1 error "declaration cannot be declared 'typedef'"
//@ diag: 21:3 error "declaration cannot be declared 'extern'"
//@ diag: 21:15 error "declaration of block scope identifier with linkage cannot have an initializer"
// [dcl.pre]/6: the decl-specifier-seq of a structured binding declaration
// may contain only static, thread_local, auto and cv-qualifiers (and, since
// P2686, constexpr/constinit: see specifiers-constexpr.cpp). An extern block
// scope declaration also cannot have an initializer. clang 19 says
// "decomposition declaration", clang 22 "structured binding declaration";
// kiln follows its -fclang-compat level (WP-377).
struct S { int a, b; };
constexpr S cs{1, 2};
inline auto [a3, b3] = cs;
static auto [a5, b5] = cs;
thread_local auto [a6, b6] = cs;
static thread_local auto [a7, b7] = cs;
typedef auto [ta, tb] = cs;
void f() {
  extern auto [e1, e2] = cs;
  static auto [s1, s2] = cs;
  thread_local auto [t1, t2] = cs;
}
