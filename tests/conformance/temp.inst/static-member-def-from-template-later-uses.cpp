// [temp.inst]/4, [temp.dep.type]/1: a static data member definition that
// a non-dependent reference in a template definition instantiates (here
// in a class template's member initializer, a function template's body
// and a default template argument) belongs to the specialization, not to
// the template being defined. Later references to the member, in template
// arguments of other templates, are not dependent after substitution:
// `lcm<...>::type` names a class and `lcm<...>::value` a value, and the
// instantiated definition is the one variable the address refers to.
// Shapes from boost::variant's alignment computation
// (`type_with_alignment`, `mpl::lcm`, `alignment_of`).
//@ expect: run
//@ strict-diags
//@ stdout: 6 6 12 4 1 1
extern "C" int printf(const char *, ...);
template <class T, T val> struct integral_constant {
  static const T value = val;
  typedef integral_constant type;
};
template <class T, T val> T const integral_constant<T, val>::value;
template <class T> struct alignment_of : integral_constant<unsigned long, sizeof(T)> {};

// Each of these instantiates the definition of
// `integral_constant<unsigned long, N>::value` while a template is defined.
template <unsigned long Align> struct X {
  static const bool b = alignment_of<char>::value >= Align;
};
template <class T> unsigned long f() { return integral_constant<unsigned long, 2>::value; }
template <class T, unsigned long N = alignment_of<int>::value> struct D {};

template <unsigned long A, unsigned long B> struct lcm {
  static const unsigned long value = A * B;
  typedef integral_constant<unsigned long, A * B> type;
};
template <typename State, typename Item>
struct apply1 : lcm<State::value, alignment_of<Item>::value>::type {};
template <typename Item>
unsigned long g() { return lcm<integral_constant<unsigned long, 2>::value, alignment_of<Item>::value>::value; }
template <typename Item>
unsigned long h() { return lcm<3, alignment_of<Item>::value>::type::value; }

typedef apply1<integral_constant<unsigned long, 6>, char>::type T1;

int main() {
  const unsigned long *p = &alignment_of<char>::value;
  const unsigned long *q = &integral_constant<unsigned long, 1>::value;
  printf("%lu %lu %lu %lu %d %d\n", T1::value, g<char[3]>(), h<int>(),
         alignment_of<int>::value, p == q, *p == 1);
}
