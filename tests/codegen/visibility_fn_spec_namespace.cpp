//@ args: -std=c++26
// Visibility of a function template's explicit specialization written in
// another definition of its namespace (spec/codegen.md §8 "Namespace
// scope"; clang getLVForNamespaceScopeDecl, mergeTemplateLV; WP-2626).
// The specialization belongs to the namespace definition it is written
// in (or its qualifier names), not its template's: one in a hidden
// definition is hidden (b), one in a `default` definition explicitly
// default. clang 22 then merges the template's own visibility, so a
// specialization of a template of a hidden namespace definition is
// hidden wherever it is written (d0..d3, k), unless it has its own
// attribute (f) or the template has one (e); clang 19 (-fclang-compat=19)
// did not (d0, d2, k default). A class or variable template's explicit
// specialization belongs to its template's definition (h, j); a member
// of a class template takes its class's visibility (i).
namespace __attribute__((visibility("hidden"))) d0 { template <class T> int g() { return 0; } }
namespace d0 { template <> int g<int>() { return 1; } }
namespace __attribute__((visibility("hidden"))) d1 { template <class T> int g() { return 0; } }
namespace __attribute__((visibility("hidden"))) d1 { template <> int g<int>() { return 1; } }
namespace __attribute__((visibility("hidden"))) d2 { template <class T> int g() { return 0; } }
namespace __attribute__((visibility("default"))) d2 { template <> int g<int>() { return 1; } }
namespace __attribute__((visibility("hidden"))) d3 { template <class T> int g(); }
template <> int d3::g<int>() { return 1; }
namespace b { template <class T> int g() { return 0; } }
namespace __attribute__((visibility("hidden"))) b { template <> int g<int>() { return 1; } }
namespace __attribute__((visibility("hidden"))) e { template <class T> __attribute__((visibility("default"))) int g() { return 0; } }
namespace e { template <> int g<int>() { return 1; } }
namespace __attribute__((visibility("hidden"))) f { template <class T> int g() { return 0; } }
namespace f { template <> __attribute__((visibility("default"))) int g<int>() { return 1; } }
namespace __attribute__((visibility("hidden"))) h { template <class T> int v = 0; }
namespace h { template <> int v<int> = 1; }
namespace __attribute__((visibility("hidden"))) i { template <class T> struct S { static int m(); }; }
namespace i { template <> int S<int>::m() { return 1; } }
namespace __attribute__((visibility("hidden"))) j { template <class T> struct S { static int m(); }; }
namespace j {
template <> struct S<int> { static int m(); };
int S<int>::m() { return 2; }
}
namespace __attribute__((visibility("hidden"))) k { namespace in { template <class T> int g() { return 0; } } }
namespace k { namespace in { template <> int g<int>() { return 1; } } }
namespace m { template <class T> int g() { return 0; } }
namespace __attribute__((visibility("default"))) m { template <> int g<int>() { return 1; } }
namespace __attribute__((visibility("hidden"))) p { struct D { template <class T> static int h(); }; }
namespace p { template <> int D::h<int>() { return 2; } }
int use() {
  return d0::g<int>() + d0::g<long>() + d1::g<int>() + d2::g<int>() + d3::g<int>() +
         b::g<int>() + e::g<int>() + f::g<int>() + h::v<int> + i::S<int>::m() +
         j::S<int>::m() + k::in::g<int>() + m::g<int>() + p::D::h<int>();
}
