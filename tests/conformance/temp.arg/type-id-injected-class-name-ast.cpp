// [temp.arg.general]/2, [dcl.ambig.res]/3, [temp.local]/1: inside its
// class template, `void(CT)` is the function type taking the
// injected-class-name, in a template argument, an alias-declaration, a
// parameter (adjusted to a pointer) and an alias of a pointer type.
//@ expect: ast
template <class T> struct W {};
template <class T> struct CT {
  using A = W<void(CT)>;
  using B = void(CT, const CT &);
  void g(void(CT));
  using P = void (*)(CT);
};
CT<int> c;
