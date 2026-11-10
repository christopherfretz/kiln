// [temp.expl.spec]/3 (CWG727): an explicit specialization of a member
// class template declared in the class template that declares the member
// template. Instantiating R<int> must instantiate the specialization as
// one, not redeclare T as a non-template.
//@ expect: run
//@ std: c++26
//@ exit: 0
//@ xfail(kiln): WP-1178 follow-up, instantiating an in-class explicit specialization of a member class template redeclares the member as a different kind of symbol
template <class Y> struct R {
  template <class X> struct T { static constexpr int t = 0; };
  template <> struct T<int> { static constexpr int t = 19; };
};
int main() { return R<int>::T<int>::t != 19 || R<int>::T<long>::t != 0; }
