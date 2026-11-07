// [dcl.ambig.res]/1, [temp.res.general]/3: `H<int>::type` names a type,
// found in the base `B<int>` of the specialization `H<int>`, so
// `int (*p)(H<int>::type)` is a declaration and `(int (*)(H<int>::type))g`
// is a cast. Deciding that during the lookahead needs `H<int>` instantiated.
// Boost.MPL `map.cpp:51` and Boost.Fusion `filter_view.cpp:117`,
// `joint_view.cpp:162`, `iterator_range.cpp:98` hit this.
//@ expect: run
//@ xfail(kiln): WP-787 follow-up, the parser lookahead does not instantiate a template-id scope, so an inherited member type is not seen as a type in a parameter list (kiln_sema decl.rs classify_template_member, kiln_parse tentative.rs after_dependent_name)
//@ stdout: 4 6
extern "C" int printf(const char*, ...);
template<class T> struct B { typedef int type; };
template<class S> struct H : B<S> {};
int g(int i) { return i * 2; }

int main() {
  int (*p)(H<int>::type) = g;
  int r = ((int (*)(H<int>::type))g)(3);
  printf("%d %d\n", p(2), r);
}
