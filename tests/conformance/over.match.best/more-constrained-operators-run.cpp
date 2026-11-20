// [over.match.best.general]/2.6, [temp.constr.order]: of two viable
// candidates otherwise equally good, the more constrained one wins, for
// member operator candidates ([over.match.oper]/3.1), `operator->`
// ([over.ref]), `operator()` and `operator[]`, non-member operator
// templates, rewritten `==` candidates ([over.match.oper]/3.4) and
// function calls; a candidate whose constraints are not satisfied is not
// viable ([over.match.viable]/3). WP-1738 (GAP-053).
//@ expect: run
//@ stdout: 1 2 1 2 3 5 7 8 1 2|1 2 1 1 1|
#include <cstdio>
template <class T> concept Big = sizeof(T) > 2;
template <class T> concept Bigger = Big<T> && sizeof(T) > 4;
struct P {
  int v;
};
template <class T> struct W {
  P p{5};
  int f() requires Big<T> { return 1; }
  int f() { return 2; }
  int operator+(int) requires Big<T> { return 1; }
  int operator+(int) { return 2; }
  int operator()() requires Bigger<T> { return 3; }
  int operator()() requires Big<T> { return 4; }
  P *operator->() requires Big<T> { return &p; }
  P *operator->() { return nullptr; }
  int operator[](int) requires Big<T> { return 7; }
  int operator[](int) { return 8; }
};
template <class T> int g(T) requires Big<T> { return 1; }
template <class T> int g(T) { return 2; }
template <class T> struct X {};
template <class T> int operator-(X<T>, int) requires Big<T> { return 1; }
template <class T> int operator-(X<T>, int) { return 2; }
template <class T> bool operator==(X<T>, int) requires Big<T> { return true; }
template <class T> bool operator==(X<T>, int) { return false; }
int main() {
  W<long long> w;
  W<char> c;
  X<long long> xl;
  X<char> xc;
  std::printf("%d %d %d %d %d %d %d %d %d %d|", w.f(), c.f(), w + 1, c + 1, w(), w->v, w[0], c[0],
              g(1LL), g('a'));
  std::printf("%d %d %d %d %d|\n", xl - 1, xc - 1, xl == 1, 1 == xl, 1 != xc);
}
