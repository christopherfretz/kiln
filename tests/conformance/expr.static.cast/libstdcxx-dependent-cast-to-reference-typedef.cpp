// [expr.static.cast]/1, /2, [temp.dep.expr]/3: a named cast in a template
// whose type is a dependent typedef of a reference type (`typedef X<R>&
// result_type;`) is an lvalue (an xvalue for an rvalue reference) before
// and after instantiation, so `static_cast<result_type>(base)` is the
// base-to-derived reference cast, not a copy into a temporary (libstdc++
// future:751 `__basic_future<_Res>::_M_get_result`; WP-332).
// Verified against clang 19.
//@ expect: run
//@ strict-diags
//@ stdout: 7 8 9
extern "C" int printf(const char *, ...);
struct B { int e; };
template<class R> struct Res : B { R r; };
template<> struct Res<void> : B { };
template<class R> struct F {
  typedef Res<R>& result_type;
  typedef Res<R>&& rref;
  result_type get(B& b) const { return static_cast<result_type>(b); }
  int move(B& b) const { return static_cast<rref>(b).e; }
};
int main() {
  Res<void> v; v.e = 7;
  Res<int> i; i.e = 8;
  F<void> fv; F<int> fi;
  B& bv = v; B& bi = i;
  fi.get(bi).e = 9;
  printf("%d %d %d\n", fv.get(bv).e, fi.move(bi) - 1, i.e);
}
