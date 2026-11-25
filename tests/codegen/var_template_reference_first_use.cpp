// [basic.def.odr]/5.1, [temp.inst]/7, [class.temporary]/6: a reference
// variable template specialization (or a reference static data member of
// a class template specialization) bound to a lifetime-extended temporary.
// The expression that first names the specialization, and so instantiates
// its definition, odr-uses it: the reference global is emitted and the use
// loads through it. A later naming finds the reference usable in constant
// expressions and folds to the `_ZGR` temporary (clang decides a
// reference's non-odr-use when it builds the name, before it instantiates
// the definition). A specialization only ever named as a constant gets no
// reference global. Namespace-scope and static member variable templates,
// const and non-const referents, constexpr and dynamic initializers, and a
// first naming inside a function template instantiation.
struct S { int a[2]; };
int g(int);

template <class T> const S &vt = S{{1, 2}};
template <class T> S &&vm = S{{3, 4}};
template <class T> constexpr const S &vc = S{{5, 6}};
template <class T> constexpr const S &vk = S{{7, 8}};
template <class T> const S &vn = S{{g(1), 2}};
struct C {
  template <class T> static const S &sm;
  template <class T> static constexpr const S &sc = S{{9, 10}};
};
template <class T> const S &C::sm = S{{11, 12}};
template <class T> struct A {
  static const S &r;
  static constexpr const S &c = S{{13, 14}};
};
template <class T> const S &A<T>::r = S{{15, 16}};

int first_vt() { return vt<int>.a[0]; }
int later_vt() { return vt<int>.a[1]; }
int first_vm() { return vm<int>.a[0]; }
int first_vc() { return vc<int>.a[0]; }
int later_vc() { return vc<int>.a[0] + vc<int>.a[1]; }
const int *later_vc_addr() { return &vc<int>.a[1]; }
constexpr int k = vk<int>.a[1];
int after_constant_vk() { return vk<int>.a[0] + k; }
int dynamic_vn() { return vn<int>.a[0]; }
int later_vn() { return vn<int>.a[1]; }
int first_sm() { return C::sm<int>.a[0]; }
int later_sm() { return C::sm<int>.a[1]; }
int first_sc() { return C::sc<int>.a[0]; }
int first_member() { return A<int>::r.a[0] + A<int>::c.a[1]; }
int later_member() { return A<int>::r.a[1] + A<int>::c.a[0]; }
template <class U> int in_template() { return vc<U>.a[0]; }
int call_in_template() { return in_template<long>(); }
void discarded() { vt<char>; }
