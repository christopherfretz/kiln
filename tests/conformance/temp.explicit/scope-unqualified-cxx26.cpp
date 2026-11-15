// [temp.explicit], [dcl.type.elab]/2, [dcl.meaning.general]/3 (WP-1448):
// since C++23 (P1787) no rule restricts where an explicit instantiation
// whose name is unqualified appears; the name is found by ordinary lookup
// (here through an enclosing namespace or a using-declaration) and the
// explicit instantiation is valid. Before C++23, C++20 [temp.explicit]/5
// rejected these (scope-unqualified-cxx20.cpp). A qualified name is still
// checked ([expr.prim.id.qual]/3, scope-qualified-not-enclosing.cpp); an
// unqualified function name only finds what is nominable in the scope.
//@ expect: compile-ok
//@ strict-diags
//@ xfail(clang): clang 19 and 22 keep the C++20 rule (err_explicit_instantiation_unqualified_wrong_namespace)
namespace N {
  template<class T> struct E {};
  inline namespace In { template<class T> struct G {}; template<class T> void h(T) {} }
  namespace Q { template<class T> struct H {}; }
}
template<class T> struct Glob {};
namespace N { inline namespace In { template struct E<int>; } }
namespace M { using N::E; template struct E<long>; }
namespace M { template struct Glob<int>; }
int main() { return 0; }
