// [class.mem.general]/11, [temp.dep.expr]: a braced default member initializer of a dependent type
// in a class template is checked when the class is instantiated (libc++'s
// __format::__retarget_buffer: `__output_buffer<_CharT> __output_{..., this}`)
//@ expect: run
//@ strict-diags
template <class C> struct Out {
  C* p; int n; void* owner;
  Out(C* p, int n, void* owner) : p(p), n(n), owner(owner) {}
};
template <class C> struct Buf {
  C storage[4] = {};
  C* begin() { return storage; }
  static constexpr int size = 4;
  Out<C> out{begin(), size, this};
  C first{storage[0]};
  int count = {size};
};
int main() {
  Buf<char> b;
  return (b.out.p == b.storage && b.out.n == 4 && b.out.owner == &b && b.count == 4) ? 0 : 1;
}
