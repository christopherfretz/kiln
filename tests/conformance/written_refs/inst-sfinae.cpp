// Per-specialization entries (spec/ast.md §18.9.3): a substitution
// failure in a function template's signature during deduction
// ([temp.deduct.general]/8) is not an instantiation and records nothing:
// `len` deduced with `Plain` fails on `t.size()` and adds no line. The
// deduction that succeeds (with `Sized`) forms the specialization, whose
// signature's `size` gets a `spec=` line naming it.
//@ written-refs: inst-sfinae.refs
struct Sized {
  int size() const { return 1; }
};
struct Plain {};

template <class T> auto len(const T &t) -> decltype(t.size()) { return t.size(); }
int len(...) { return 0; }

int run() { return len(Sized{}) + len(Plain{}); }
