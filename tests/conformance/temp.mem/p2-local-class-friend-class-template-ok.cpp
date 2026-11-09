// [temp.mem]/2 valid twin: a local class may befriend a class template
// specialization or a non-template class; a non-local class may befriend
// a class template (WP-1041).
//@ expect: compile-ok
template<class T> struct X {};
struct C;
template<class T> class Y;
struct G { template<class T> friend class Y; };
void f() {
  struct L {
    friend struct X<int>;
    friend struct C;
  };
  L l;
  (void)l;
}
