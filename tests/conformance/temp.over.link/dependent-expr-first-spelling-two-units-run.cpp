// [temp.over.link]/5, Itanium ABI §5.1.5 (<template-arg> X <expression>
// E): `R<T>::value` and `R<T, void>::value` (a defaulted argument) are
// equivalent expressions, so each pair of declarations below names one
// enclosing dependent type. clang mangles the expression as the
// declaration that first formed that type spelled it: per enclosing
// type (`EI<..., T>` and `EI<..., T *>` here; a `decltype` and an array
// bound each count as one too), not per expression. The templates are
// only declared here; the other unit (Inputs/dependent-expr-first-
// spelling-defs.cpp) defines each specialization under clang's name with
// an asm label, so the program links only if this unit's references
// carry those names. kiln used the first spelling of the expression
// anywhere in the TU, so `f2`/`g2`, `dh` and `al` took `R<T>` (WP-790).
//@ expect: run
//@ args: %{test_dir}/Inputs/dependent-expr-first-spelling-defs.cpp
//@ stdout: 11 12 13 14 15 16 17 18
extern "C" int printf(const char *, ...);

template <class, class = void> struct R { static constexpr int value = 1; };
template <int, class T> struct EI { using type = T; };
template <int> struct A {};

template <class T> typename EI<R<T>::value, T>::type f(T);
template <class T> typename EI<R<T, void>::value, T>::type g(T);
template <class T> typename EI<R<T, void>::value, T *>::type g2(T);
template <class T> typename EI<R<T>::value, T *>::type f2(T);
template <class T> int dm(A<R<T>::value + 1>);
template <class T> int dg(decltype(R<T, void>::value + 1));
template <class T> int dh(decltype(R<T>::value + 1));
template <class T> int ak(A<0>, int (*)[R<T, void>::value + 1]);
template <class T> int al(int (*)[R<T>::value + 1]);

int main() {
  printf("%d %d %d %d %d %d %d %d\n", f(1), g(2), *g2(3), *f2(4), dm<int>({}), dg<int>(0),
         dh<int>(0), ak<int>({}, nullptr) + al<int>(nullptr) - 18);
}
