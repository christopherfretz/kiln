// spec/contracts.md (P2900R14): 10,000 functions, each with a
// precondition, a postcondition naming the result, and a contract_assert
// in the body, expanded from nested macros so the source stays small.
// Pins that parsing, sema (the const-ification of every id-expression in
// a predicate) and the redeclaration check scale linearly: each function
// is declared, then redeclared without its sequence, then defined. A
// sequence costs one AST node per specifier, so the budget is a few
// times that of the same functions without contracts. Nothing is
// evaluated: no Oven work under the default (enforce) semantic. GCC 16.1
// accepts it but needs ~300 MiB, over the budget, so there is no oracle.
// WP-706 measured (debug kiln, -fsyntax-only): 5.6 s and 48 MiB RSS
// against 2.8 s and 39 MiB with the contracts stripped; 1,000 functions
// take 0.58 s (linear).
//@ expect: compile-ok
//@ skip(clang): clang 22 does not implement P2900 contracts
//@ oven: evaluations<=0 steps<=0 calls<=0
//@ mem<=256
#define D(n)                                                           \
  int f##n(const int x) pre(x >= 0) post(r : r > x);                   \
  int f##n(const int x);                                               \
  int f##n(const int x) pre(x >= 0) post(r : r > x) {                  \
    contract_assert(x < 1000000);                                      \
    return x + n;                                                      \
  }
#define A(p) D(p##0) D(p##1) D(p##2) D(p##3) D(p##4) D(p##5) D(p##6) D(p##7) D(p##8) D(p##9)
#define B(p) A(p##0) A(p##1) A(p##2) A(p##3) A(p##4) A(p##5) A(p##6) A(p##7) A(p##8) A(p##9)
#define C(p) B(p##0) B(p##1) B(p##2) B(p##3) B(p##4) B(p##5) B(p##6) B(p##7) B(p##8) B(p##9)
#define E(p) C(p##0) C(p##1) C(p##2) C(p##3) C(p##4) C(p##5) C(p##6) C(p##7) C(p##8) C(p##9)
E(1)
int use() { return f10000(1) + f19999(2); }
