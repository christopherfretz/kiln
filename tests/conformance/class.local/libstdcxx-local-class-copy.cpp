// [class.local], [class.copy.ctor]/7: a local class declared inside a member
// function template of a class template has an implicit copy constructor.
// libstdc++ bits/shared_ptr_base.h:789 (_Sp_counted_array_base::_Iter);
// <memory>, <future>, <chrono>, 7 headers (docs/status/libstdcxx.md, WP-188).
// Verified against clang 19.
//@ expect: compile-ok
//@ xfail(kiln): no matching constructor for copy-init of a local class in a member template
template<class T> struct base {
  unsigned n = 3;
  template<class U> unsigned fill(const U *init) {
    struct It {
      const U *p;
      unsigned pos;
      It &operator++() { ++pos; return *this; }
      bool operator==(const It &i) const { return pos == i.pos; }
    };
    It first{init, 0};
    It last = first;
    last.pos = n;
    return last.pos - first.pos;
  }
};
unsigned g() { int a[1] = {}; return base<long>().fill(a); }
