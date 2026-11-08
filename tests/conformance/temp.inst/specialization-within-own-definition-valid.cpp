// [temp.dep.type]/1, [temp.local]/1, [class.mem.general]/7: inside a class
// template's definition the current instantiation (`Q<T>::`, `Q::`) is
// looked up without instantiating, a pointer to another specialization
// needs no complete type, and a complete-class context (a member function
// body, a default member initializer, a default argument) sees the class
// complete, so another specialization instantiates there
// ([temp.inst]/2). The ill-formed uses:
// specialization-within-own-definition-{member-typedef,field}.cpp.
//@ expect: compile-ok
//@ strict-diags
template <class R> struct ptr { using string_t = R; };
template <class T> struct Q {
  using jp = ptr<T>;
  using u = Q::jp;
  using v = typename Q<T>::jp;
  using w = Q<T *> *;
  int n = sizeof(Q<char>);
  int f(int k = sizeof(Q<short>)) { return k; }
  static int g() {
    typename Q<long>::jp::string_t x = 1;
    return static_cast<int>(x);
  }
};
Q<int> q;
int r = q.f() + Q<char>::g();
