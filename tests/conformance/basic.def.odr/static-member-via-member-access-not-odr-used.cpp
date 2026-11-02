// [basic.def.odr]/4-5: a `static const` integral member read through a
// member access (`b.w`, `this->b.w`, `p->w`) is not an odr-use when it is
// usable in constant expressions; the read is its value, the base is still
// evaluated for its side effects. No out-of-line definition exists, so the
// program links only if every read folds (libc++ `__shared_mutex_base::
// __write_entered_`, try_lock_until.pass.cpp). Matches clang 19.
//@ expect: run
//@ stdout: 2147483649 2147483648 3 1
extern "C" int printf(const char*, ...);
struct B {
  static const unsigned w = 1u << 31;
  static const int n = 3;
  unsigned s = 1;
};
struct M {
  B b;
  unsigned f() {
    b.s |= b.w;
    return b.s;
  }
  unsigned g() const { return this->b.w; }
};
int calls = 0;
B* get(B* p) {
  ++calls;
  return p;
}
int main() {
  M m;
  B x;
  unsigned a = m.f();
  int n = get(&x)->n;
  printf("%u %u %d %d\n", a, m.g(), n, calls);
}
