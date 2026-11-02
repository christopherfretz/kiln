// [temp.arg.explicit]/1, [temp.deduct.general]/5, [temp.inst]/3: a call of
// a member function template in a template, resolved at the definition by
// deduction, is resolved again at instantiation from the arguments alone.
// The deduced arguments are not written: as explicit arguments they would
// also go to the other templates of the overload set, and substituting
// `char[1]` into `str(const Str<C, A>&)` would instantiate `Str<char,
// char[1]>` outside the immediate context (libc++ `stringstream::str("")`).
//@ expect: run
//@ strict-diags
template <class C, class A> struct Str {
  static_assert(sizeof(A) != 1, "allocator must not be char[1]");
  C c;
};
template <class C> struct Stream {
  template <class A> int str(const Str<C, A>&) { return 1; }
  template <class T> int str(const T&) { return 2; }
  int str(const Str<C, int>&) { return 3; }
};
template <class T> int g(Stream<char>& s) { return s.str(""); }
template <class T> struct H {
  Stream<char> s;
  int f() { return s.str(""); }
};
int main() {
  Stream<char> s;
  if (g<int>(s) != 2)
    return 1;
  if (H<int>{}.f() != 2)
    return 2;
  return 0;
}
