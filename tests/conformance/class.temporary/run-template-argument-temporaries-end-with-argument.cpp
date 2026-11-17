// [intro.execution]/5, [class.temporary]/4: the temporaries of a template
// argument end with it (it is a full-expression of its own), and that
// takes none of the enclosing full-expression's: `D(n)` is destroyed at the
// end of the full-expression that created it, whether the template argument
// before or after it is dependent, converted later by overload resolution,
// or an argument of a dependent template name; a default argument's
// temporary following a dependent template argument is destroyed at the
// end of the call. The rule behind
// template-argument-temporaries-end-with-argument-cxx14.cpp (WP-1578).
//@ args: -std=c++14
//@ expect: run
//@ stdout: +1 use1 end -1 | +2 use2 end -2 | +3 h1 end -3 | +4 use4 +5 h2 end -5 -4 | +6 body -6 | +7 body -7 | +8 use8 end -8 |
extern "C" int printf(const char*, ...);
struct L { int v; };
constexpr int one(L l) { return l.v; }
struct D {
  int id;
  D(int i) : id(i) { printf("+%d ", id); }
  ~D() { printf("-%d ", id); }
};
int use(const D& d) { printf("use%d ", d.id); return d.id; }
template <int N> struct C { static constexpr int v = N; };
template <int N> int h(const D& d) { printf("h%d ", N); return d.id; }
template <bool B, class T> struct enable_if {};
template <class T> struct enable_if<true, T> { using type = T; };
struct Nil {};

template <int N> int f1() { return use(D(1)) + C<one(L{N})>::v + (printf("end "), 0); }
template <int N> int f2() { return C<one(L{N})>::v + use(D(2)) + (printf("end "), 0); }
template <int N> int f3() { return h<one(L{N})>(D(3)) + (printf("end "), 0); }
template <int N> int f4() { return use(D(4)) + h<one(L{N + 1})>(D(5)) + (printf("end "), 0); }
template <int N = 0>
int f5(typename enable_if<one(L{N}) == 0, Nil>::type = {}, const D& d = D(6)) {
  printf("body ");
  return d.id;
}
template <class T>
int f6(typename T::template X<one(L{1})>::type = {}, const D& d = D(7)) {
  printf("body ");
  return d.id;
}
struct HasX { template <int> struct X { using type = int; }; };
int f8() { return use(D(8)) + C<one(L{1})>::v + (printf("end "), 0); }
int main() {
  f1<1>(); printf("| ");
  f2<1>(); printf("| ");
  f3<1>(); printf("| ");
  f4<1>(); printf("| ");
  f5(); printf("| ");
  f6<HasX>(); printf("| ");
  f8(); printf("|\n");
}
