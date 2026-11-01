// [namespace.udecl]/1, /4, /15: a using-declarator may name an operator
// function or a conversion function of a base, also through a dependent
// base; a member declared in the derived class with the same name and
// parameter-type-list hides the one the using-declaration introduces
//@ expect: run
//@ strict-diags
struct B {
  int v = 0;
  B& operator=(int x) { v = x; return *this; }
  int operator()(int a) const { return a + v; }
  int operator[](int) const { return 1; }
  explicit operator bool() const { return v != 0; }
};
struct D : B {
  using B::operator=;
  using B::operator();
  using B::operator[];
  using B::operator bool;
  int operator[](int) const { return 2; }
};
template <class T> struct TB { T v{}; TB& operator=(T x) { v = x; return *this; } };
template <class T> struct TD : TB<T> { using TB<T>::operator=; };
template <class T> struct CB { operator T() const { return T(3); } };
template <class T> struct CD : CB<T> { using CB<T>::operator T; };
int main() {
  D d;
  d = 3;
  int r = d(4);
  if (d) r++;
  if (r != 8) return 1;
  if (d[0] != 2) return 2;
  TD<int> t;
  t = 5;
  TD<int> t2;
  t2 = t;
  if (t2.v != 5) return 3;
  CD<long> c;
  long l = c;
  if (l != 3) return 4;
  return 0;
}
