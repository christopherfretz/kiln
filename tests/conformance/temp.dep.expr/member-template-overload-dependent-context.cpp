// [temp.dep.expr]/3, [temp.dep.candidate], [temp.deduct]/8: in a member of
// a class template, an unqualified name for an overload set of member
// function templates of that class (a dependent context) is
// type-dependent, even when the call's arguments are not, so the call
// `to_char(u)` is resolved at instantiation. There, the defaulted
// template argument `C = T` and the SFINAE constraint on it leave one
// viable candidate per specialization; resolving it at the definition,
// where `C` is still `T` and both constraints are unknown, reported the
// call as ambiguous. clang's `OverloadExpr` dependence: a decl whose
// context is dependent. nlohmann/json `binary_writer::to_char_type`
// (WP-668). Verified against clang 19.
//@ expect: run
template <bool B, class T = void> struct enable_if {};
template <class T> struct enable_if<true, T> { using type = T; };
template <class T> struct is_int { static constexpr bool value = false; };
template <> struct is_int<int> { static constexpr bool value = true; };

template <class T> struct writer {
  template <class C = T, typename enable_if<is_int<C>::value>::type* = nullptr>
  static int to_char(unsigned char x) { return x + 1; }
  template <class C = T, typename enable_if<!is_int<C>::value>::type* = nullptr>
  static int to_char(unsigned char x) { return x + 2; }
  int put() {
    unsigned char u = 10;
    return to_char(u);
  }
  static int put_static() { return writer::to_char(20); }
  int out_of_line();
};

template <class T> int writer<T>::out_of_line() { return to_char(30); }

// Never instantiated: the set is dependent, so nothing is diagnosed here.
template <class T> struct never {
  template <class C = T> static void g(int);
  template <class C = T> static void g(long);
  void h() { g(0u); }
};

int main() {
  writer<int> a;
  writer<long> b;
  return a.put() == 11 && b.put() == 12 && writer<int>::put_static() == 21
                 && writer<long>::put_static() == 22 && a.out_of_line() == 31
                 && b.out_of_line() == 32
             ? 0
             : 1;
}
