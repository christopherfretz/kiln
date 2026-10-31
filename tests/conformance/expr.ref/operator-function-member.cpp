// [expr.ref]/1, [expr.prim.id.unqual]/1: a member access may name an
// operator-function-id or a conversion-function-id; an unqualified
// operator-function-id in a member function is an implicit `this->`.
//@ expect: run
//@ stdout: 3 4 7 7 9 | 5 5
extern "C" int printf(const char*, ...);

struct Acc {
  int v = 0;
  constexpr Acc* operator->() { return this; }
  constexpr Acc& operator+=(int x) { v += x; return *this; }
  constexpr int operator()() const { return v; }
  constexpr operator int() const { return v; }
  constexpr int twice() const { return operator()() + operator int(); }
};

constexpr int use(Acc a) {
  a.operator+=(2);
  Acc* p = &a;
  p->operator+=(1);
  return a.operator()() + a.operator->()->v + p->operator int();
}
static_assert(use(Acc{}) == 9);
static_assert(Acc{4}.twice() == 8);

template <class T> int generic(T t) {
  t.operator+=(5);
  return t.operator->()->v;
}

int main() {
  Acc a;
  a.operator+=(3);
  int x = a.operator int();
  a.operator+=(1);
  printf("%d %d %d %d %d | ", x, a.operator()(), a.twice() - 1, a.operator->()->v + 3, use(Acc{}));
  printf("%d %d\n", generic(Acc{}), generic<Acc&>(a) - 4);
}
