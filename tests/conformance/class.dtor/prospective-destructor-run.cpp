// [class.dtor]/4: the selected prospective destructor is the one called at
// the end of an object's lifetime ([class.dtor]/14); the unselected one is
// never called. Verified against clang 19.
//@ expect: run
//@ strict-diags
int count = 0;
template <class T> struct W {
  ~W() requires (sizeof(T) > 4) { count += 10; }
  ~W() { count += 1; }
  T t;
};
int main() {
  { W<char> a; W<long> b; }
  W<long> *p = new W<long>;
  delete p;
  return count == 21 ? 0 : 1;
}
