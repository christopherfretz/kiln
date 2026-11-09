// Coverage zero records of class-template members ([temp.inst]/11,
// [temp.explicit]/12; WP-1107, spec/codegen.md §5.4 "Instrumentation"):
// clang defers an unused function's record when the parser finishes an
// inline definition (`HandleInlineFunctionDefinition`, never for a
// member of a specialization: those are instantiated, not parsed) and
// when an instantiation ends (`InstantiateFunctionDefinition`'s
// `HandleTopLevelDecl`, before its own pending instantiations run), and
// emits the records still unused at the end of the TU in that order.
// Members used out of order, an `extern template` specialization, nested
// member class templates and member function templates.
//@ args: -fprofile-instr-generate -fcoverage-mapping -fcoverage-compilation-dir=/cov -Xclang -disable-llvm-passes
template <class T> struct S {
  int a() { return 1; }
  int b() { return 2; }
  constexpr int c() const { return 3; }
  int d();
  static int e() { return 4; }
  template <class U> struct In {
    int f() { return 5; }
    static int g() { return U(6); }
    template <class V> int h(V v) { return v; }
  };
};
template <class T> int S<T>::d() { return a() + 7; }
template <class T> int ft(T x) { return S<T>().b() + x; }
template <class T> constexpr int cf(T x) { return x; }
template <class T> inline int it(T x) { return x + 1; }
extern template struct S<long>;
extern template int it(long);
static int uses() {
  S<long> s;
  int r = s.d() + S<long>::e() + s.b();
  r += S<long>::In<char>::g() + S<long>::In<int>().f();
  r += S<long>::In<int>().h(2.0);
  r += it(3L);
  r += ft(4.0f);
  r += S<unsigned>::In<short>::g() + S<unsigned>().d();
  constexpr int k = S<long>().c() + cf(5);
  return r + k + S<unsigned>().e();
}
int other() { return S<short>().b(); }
