//@ mode: emit-llvm
//@ args: -freflection
// Emitting 2,000 specializations of a function template on a
// std::meta::info template argument (spec/reflection.md §8.3; WP-485,
// WP-501): spec<^^C0000> ... spec<^^C1999>, each with a function-local
// static, so every specialization writes the reflection literal
// `L Dm ty <type> E` (the default gcc form) into its function symbol and its static's
// symbol. The literal is the reflectee's encoding (one type name here),
// so the cost is the instantiation and the IR, linear in the count. The table of addresses
// odr-uses every specialization. kiln only: clang 19/22 lack `^^`.

using info = decltype(^^int);

template <info R> int spec() {
  static int n = 0;
  return ++n;
}

#define T10(m, p) m(p##0) m(p##1) m(p##2) m(p##3) m(p##4) m(p##5) m(p##6) m(p##7) m(p##8) m(p##9)
#define T100(m, p)                                                                  \
  T10(m, p##0) T10(m, p##1) T10(m, p##2) T10(m, p##3) T10(m, p##4) T10(m, p##5)    \
  T10(m, p##6) T10(m, p##7) T10(m, p##8) T10(m, p##9)
#define T1000(m, p)                                                                 \
  T100(m, p##0) T100(m, p##1) T100(m, p##2) T100(m, p##3) T100(m, p##4)            \
  T100(m, p##5) T100(m, p##6) T100(m, p##7) T100(m, p##8) T100(m, p##9)

#define DECL(n) struct C##n {};
T1000(DECL, 0)
T1000(DECL, 1)

#define ADDR(n) &spec<^^C##n>,
int (*const table[])() = {T1000(ADDR, 0) T1000(ADDR, 1)};

int run() {
  int s = 0;
  for (auto f : table) s += f();
  return s;
}
