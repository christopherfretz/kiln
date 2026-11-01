// [class.dtor]/1, [expr.prim.id.dtor]: an explicit destructor call may name
// the class with a simple-template-id, in and out of a template
//@ expect: run
//@ strict-diags
int destroyed = 0;
template <class T, class U> struct Pair { T a; U b; ~Pair() { ++destroyed; } };
template <class T> struct Holder {
  union { Pair<T, int> p; };
  Holder() : p{} {}
  ~Holder() { p.~Pair<T, int>(); }
};
union Plain {
  Pair<int, int> p;
  Plain() : p{} {}
  ~Plain() { Pair<int, int>* q = &p; q->~Pair<int, int>(); }
};
int main() {
  { Holder<long> h; }
  { Plain u; }
  return destroyed == 2 ? 0 : 1;
}
