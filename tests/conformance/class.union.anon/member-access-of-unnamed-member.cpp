// [class.union.anon]/1: a member of an anonymous union declared in a
// class is accessed with the access of the unnamed member, not the
// (public) access it has inside the union; nested anonymous unions take
// the outermost one's.
// Found running GCC's C++ testsuite.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 32:5 error "'hidden' is a private member of 'Holder'"
//@ diag: 20:9 note "declared private here"
//@ diag: 33:12 error "'inner' is a private member of 'Holder'"
//@ diag: 22:11 note "declared private here"
class Holder {
public:
  union {
    int open;
  };
  int get() { return hidden + inner; }
private:
  union {
    int hidden;
    union {
      int inner;
    };
  };
  friend int peek(Holder &);
};

int peek(Holder &h) { return h.hidden + h.inner; }

int use(Holder &h) {
  h.open = 1;
  h.hidden = 2;
  return h.inner;
}
