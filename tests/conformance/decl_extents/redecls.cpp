// The declaration-extent table (spec/ast.md §19.1): every written
// declaration has its own row, also when the AST keeps it under the
// DeclId of the entity it redeclares or names. Explicit instantiations
// of a function template (with and without template arguments,
// [temp.explicit]/8), a variable template, a member function, a static
// data member and a constructor of a class template specialization
// ([temp.explicit]/3) each have a row keyed by the specialization or
// member; an `extern template` declaration and the later explicit
// instantiation definition ([temp.explicit]/2), repeated explicit and
// partial specialization declarations ([temp.expl.spec]/1,
// [temp.spec.partial.general]/1) and repeated opaque enumeration
// declarations ([dcl.enum]/3) keep one row each, in source order. A
// friend naming a specialization declared before it adds no row
// ([class.friend]/2). Init-captures in every form have a row
// ([expr.prim.lambda.capture]/6).
//@ decl-extents: redecls.extents
//@ std: c++26
template <class T> void t(T) {}
template <class T> T vt = T();
template <class T> struct S {
  S() {}
  void m() {}
  static int sm;
};
template <class T> int S<T>::sm = 0;
template void t<int>(int);
template void t(short);
template int vt<int>;
template void S<long>::m();
template int S<long>::sm;
template S<long>::S();
extern template struct S<char>;
template struct S<char>;
extern template void t<long>(long);
template void t<long>(long);
template <class T> struct Q {};
template <> struct Q<int>;
template <> struct Q<int>;
template <> struct Q<int> {};
template <class T> struct Q<T *>;
template <class T> struct Q<T *> {};
struct F {
  friend struct Q<int>;
};
enum class E : int;
enum class E : int;
enum class E : int { a };
class K;
class K;
struct A;
struct B {
  friend struct A;
};
int target = 0;
template <class... Ts> int pack(Ts... args) {
  auto l = [... xs = args, &r = target] { return (r + ... + xs); };
  return l();
}
struct M {
  int k = [y{2}, z(3)] { return y + z; }();
  void m(int q = [w = 4] { return w; }()) { (void)q; }
};
auto outer = [a = 1] { return [b = a] { return b; }(); };
int u = pack(1, 2);
