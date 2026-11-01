//@ expect: ast
struct A {
  ~A();
  int operator+=(int);
  operator int() const;
  A* operator->();
  int call() { return operator+=(1) + operator int(); }
};
typedef int I;

void use(A a, A* pa, I* pi, int i) {
  a.~A();
  pa->~A();
  a.operator+=(2);
  pa->operator int();
  a.operator->();
  pi->~I();
  i.~I();
}

template <class T> void destroy(T* p) { p->~T(); }
template <class T> int ops(T t) { return t.operator+=(1) + t.operator int(); }
