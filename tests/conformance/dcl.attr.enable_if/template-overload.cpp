// clang's `enable_if` attribute (clang AttrDocs "enable_if", `Sema::IsOverload`;
// [temp.over.link]/5, [over.match.best] as clang extends it): function
// templates, member templates and constructor templates that differ only in
// their `enable_if` conditions are overloads; the conditions are evaluated
// after deduction.
//@ expect: run
//@ strict-diags
template <class T> T twice(T x) { return x + x; }
template <class T> T twice(T x) __attribute__((enable_if(x == 0, "zero"))) { return 9; }

struct W {
  template <class T> W(T) : v(1) {}
  template <class T> W(T t) __attribute__((enable_if(t == 5, "five"))) : v(2) {}
  template <class T> int m(T t) { return int(t); }
  template <class T> int m(T t) __attribute__((enable_if(t < 0, "neg")));
  int v;
};
template <class T> int W::m(T t) __attribute__((enable_if(t < 0, "neg"))) { return 77; }

template <class T> struct C {
  int h(int n) { return n; }
  int h(int n) __attribute__((enable_if(n == 3, "three"))) { return 33; }
  C() : k(0) {}
  C(int n) __attribute__((enable_if(n == 4, "four"))) : k(44) {}
  C(int n) : k(n) {}
  int k;
};

int main() {
  if (twice(0) != 9 || twice(2) != 4 || twice(0L) != 9) return 1;
  if (W(5).v != 2 || W(6).v != 1) return 2;
  W w(0);
  if (w.m(-1) != 77 || w.m(4) != 4) return 3;
  C<int> c;
  if (c.h(3) != 33 || c.h(1) != 1) return 4;
  if (C<int>(4).k != 44 || C<int>(8).k != 8) return 5;
  return 0;
}
