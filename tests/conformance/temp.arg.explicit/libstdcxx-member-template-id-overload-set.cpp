// [temp.arg.explicit]/4, [over.match.call.general]/1, [class.mfct.non.static]:
// an unqualified member template-id `f<A>(args)` names the whole overload
// set lookup finds, non-template member functions included (which are
// then not viable with explicit template arguments); every member function
// template is a candidate (libstdc++ bitset:1358 `to_string`, WP-323).
// Verified against clang 19.
//@ expect: run
//@ stdout: 97 0 98
extern "C" int printf(const char *, ...);

template<int N> struct bits {
  template<class C> int to_s() const { return 0; }
  template<class C> int to_s(C z, C o = C('1')) const { return z + o; }
  int to_s(char z, char o = '1') const { return to_s<char>(z, o); }
  int to_s() const { return to_s<char>(); }
};

struct plain {
  int g(char) const { return 1; }
  template<class C> int g() const { return 0; }
  template<class C> int g(C a, C b) const { return a + b; }
  int h() const { return g<char>('1', '1'); }
};

int main() {
  bits<3> b;
  printf("%d %d %d\n", b.to_s('0'), b.to_s(), plain{}.h());
}
