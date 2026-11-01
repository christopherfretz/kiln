//@ expect: ast
// [temp.expl.spec]/1, [temp.inst]/3: the members of an implicit
// specialization explicitly specialized; a scoped member enumeration of the
// specialization stays a declaration until required (here X<long>::F),
// unlike an unscoped one, defined with its class
template <class T>
struct X {
  struct N { T a; };
  enum class F : int { a, b };
  enum U : int { u };
  enum class G : int;
};
template <>
struct X<int>::N { long z; };
template <>
enum class X<int>::F : int { p = 10 };
template <>
enum class X<int>::G : int { g };
X<long> xl;
X<int>::N xn;
int xf = int(X<int>::F::p);
