// The definitions of the context-independence test (WP-1586; spec/harness.md
// "Context independence of kind 2 rows"): one of each kind that gets a kind 2
// row and one using each area of the definition hash (spec/abi.md
// "Definition hash"). No system header: the preambles bring their own.
// C++20 and later; the contract only from C++26.
#ifndef KILN_MASON_CONTEXT_DEFS_H
#define KILN_MASON_CONTEXT_DEFS_H
namespace hp {
enum Color { Red, Green = 4 };
enum class Mode : short { A = 1, B };
enum { Anon = 7 };
struct Base { int b = 1; virtual ~Base() {} virtual int v() const { return b; } };
struct Derived : Base {
  int d;
  Derived() : d(2) {}
  Derived(int x) : Base(), d(x) {}
  Derived(const Derived &) = default;
  ~Derived() override {}
  int v() const override { return d + b; }
};
struct Agg { int a; int b = 7; };
struct Dm { int a = [] { return 3; }(); };
// Declarators, default arguments, enumerators.
inline int plain(int a, int b = 3) { return a * b + Green + Anon; }
inline int isB(int e) { return e == static_cast<int>(Mode::B); }
// Closures: captures, a nested closure, a generic one, one escaping.
inline int lam(int y) { int k = 2; auto f = [=, &y](int x) mutable { y += x * k; return [y] { return y; }(); }; return f(1); }
inline int generic(int z) { auto g = [z](auto u) { return u + z; }; return g(1) + (int)g(2L); }
inline auto mk() { int c = 1; return [c](int x) { return x + c; }; }
inline int use_mk() { auto f = mk(); return f(1); }
inline auto lv = [](int x) { return x * 2; };
inline int dflt(int x = [] { return 4; }()) { return x; }
// Local classes and enumerations, structured bindings, statics.
inline int local_cls(int v) { struct L { int m; int get() const { return m + 1; } }; enum { E1 = 7 }; return L{v}.get() + E1; }
inline int structured() { struct P { int x, y; } p{1, 2}; auto [a, b] = p; return a + b; }
inline int statics() { static int s = 4; return ++s; }
// Templates: members of class templates, a member template, a dependent
// noexcept, a static data member, a variable template, a function template
// with a dependent signature and a constraint.
template <class T> struct Box {
  T t{};
  T get() const { return t; }
  static int counter;
  template <class U> U conv(U u) const { return u + U(t); }
  T noex() const noexcept(noexcept(T())) { return t; }
  virtual T virt() noexcept(sizeof(T) > 1) { return t; }
};
template <class T> int Box<T>::counter = 5;
template <class T> constexpr T vt = T(3);
template <class T> T twice(T t) { return t + t; }
template <class T> auto tsig(T t) -> decltype(t + 1) requires(sizeof(T) > 1) { return t + 1; }
template <class T> inline int tlam() { return [](T t) { return (int)t; }(T()); }
inline int callees() {
  Box<long> b;
  return twice(1) + (int)twice(2L) + (int)b.get() + b.conv(1) + vt<int> + Box<int>::counter + (int)b.noex()
       + (int)b.virt() + tsig(2L) + tlam<int>() + tlam<char>();
}
// Attributes, literals, placeholders.
#if __cplusplus > 202302L
inline int contract(int x) pre(x > 0) { contract_assert(x != 3); return x; }
#else
inline int contract(int x) { return x; }
#endif
[[noreturn]] inline void dies() { throw 1; }
inline int attrs(int x) { [[assume(x > 0)]]; alignas(16) int a = x; return a; }
alignas(8) inline int aligned_var = 2;
inline const char *str() { return "abc"; }
inline long lit() { return 0x10 + 16u + 'c' + (long)2.5; }
inline int func_name() { return __func__[0] + __LINE__; }
// Initialization, new and delete, exceptions, casts.
inline int news() { int *p = new int[3]{1, 2, 3}; int r = p[1]; delete[] p; return r; }
inline int tries() { try { throw Derived(1); } catch (const Base &b) { return b.v(); } catch (...) { return 0; } }
inline int agg() { Agg g{1}; return g.a + g.b + Dm().a; }
inline int dyncast(Base *b) { return dynamic_cast<Derived *>(b) ? 1 : 0; }
inline int sizes() { return sizeof(Derived) + alignof(Base) + (int)noexcept(plain(1)); }
inline int dflt_member(const Derived &d = Derived()) { return d.d; }
// Statements, GNU forms.
inline int loops(int n) {
  int s = 0;
  for (int i = 0; i < n; ++i) { if (i % 2) continue; s += i; }
  int a[3] = {1, 2, 3};
  for (int x : a) s += x;
  while (s > 100) s /= 2;
  switch (n) { case 1: return 1; default: break; }
  return s;
}
inline int stmt_expr() { return ({ int q = 3; q + 1; }); }
inline int gotos(int n) { if (n) goto out; n = 5; out: return n; }
inline int builtins(int x) { int r; __atomic_load(&x, &r, __ATOMIC_SEQ_CST); return r + __builtin_offsetof(Agg, b); }
// Constant evaluation, deduced types, variables.
constexpr int cexpr(int x) { return x * 2; }
consteval int ceval(int x) { return x + 1; }
inline int uses_ce() { return ceval(1) + cexpr(2); }
inline auto deduced() { return Box<char>{}.get(); }
inline int ivar = cexpr(4);
inline thread_local int tvar = 1;
inline int (Box<int>::*pmf)() const = &Box<int>::get;
} // namespace hp
inline int use_all() {
  hp::Derived d;
  hp::Base *b = &d;
  return hp::plain(1) + hp::isB(1) + hp::lam(1) + hp::generic(1) + hp::use_mk() + hp::lv(1) + hp::dflt()
       + hp::local_cls(1) + hp::structured() + hp::statics() + hp::callees() + hp::contract(1) + hp::attrs(1)
       + hp::aligned_var + *hp::str() + (int)hp::lit() + hp::func_name() + hp::news() + hp::tries() + hp::agg()
       + hp::dyncast(b) + hp::sizes() + hp::dflt_member() + hp::loops(3) + hp::stmt_expr() + hp::gotos(1)
       + hp::builtins(1) + hp::uses_ce() + hp::deduced() + hp::ivar + hp::tvar + (hp::Box<int>().*hp::pmf)()
       + (b->v() > 100 ? (hp::dies(), 0) : 0);
}
#endif
