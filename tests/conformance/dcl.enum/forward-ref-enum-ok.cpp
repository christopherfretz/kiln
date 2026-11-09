// [dcl.enum]/2, [dcl.type.elab]/3: valid twin of forward-ref-enum.cpp. An
// opaque declaration with an enum-base, a scoped one (its underlying type
// is int), and an elaborated `enum E` or redeclaration naming an
// enumeration already declared that way or already defined are not
// forward references. WP-1078.
//@ std: c++17
//@ expect: compile-ok
//@ strict-diags

enum F : int;
enum class G;
enum struct H : long;
enum F *pf;
enum G *pg;
enum F : int { f1 };
enum E { e };
enum E;
enum E *pe;
void f(enum E);
typedef enum E Et;
struct S {
  enum M : short;
  enum M *pm;
};
enum S::M : short { m };
template <class T> void h() {
  enum V : int;
  enum V *pv = nullptr;
  (void)pv;
}
template void h<int>();
template <class T> struct X {
  enum Q : T;
  enum Q *q;
};
X<char> xc;
namespace N { enum class W; }
enum class N::W { w };
enum N::W *pw;
