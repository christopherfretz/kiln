//@ expect: run
//@ std: c++26
// [conv.ptr]/3, [class.mi]/4: a conversion to a virtual base finds the
// base subobject through the most derived object. Code generation tries
// to fold `sum`'s loop condition, a conversion of `in` to its virtual
// base, although no object of `basic_istream<char>` is constructed in
// this translation unit (`in` is only declared, weak, so the program
// links without it): the subobject graph of the class must still be
// known. Reduced from real code (a stream read in a loop condition).
struct ios_base {
  int st = 0;
};
template <class C> struct basic_ios : ios_base {
  explicit operator bool() const { return st == 0; }
};
template <class C> struct basic_istream : virtual basic_ios<C> {
  basic_istream &operator>>(int &n) {
    n = 1;
    this->st = 1;
    return *this;
  }
};
extern basic_istream<char> in [[gnu::weak]];
int sum() {
  int s = 0;
  for (int n; in >> n;)
    s += n;
  return s;
}
int main() { return &in == nullptr ? 0 : sum(); }
