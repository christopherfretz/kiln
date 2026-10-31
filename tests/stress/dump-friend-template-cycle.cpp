//@ expect: ast
//@ mem<=512
//@ xfail(kiln): in an instantiation, kiln dumps the befriended template's pattern (U at depth 1, no specialization list) where clang dumps it instantiated (depth 0, the specializations); the memory budget is checked regardless (KILLED is never masked)
// [temp.friend]/1, spec/ast-dump.md: a class template that befriends
// itself, dumped with its instantiations. Each befriending redeclares the
// template inside its own instantiation, so the friend's declaration leads
// back to the enclosing class. Regression test (WP-125, fixed in WP-121):
// on 2026-10-31 `kiln -fdump-ast` followed that cycle and allocated without
// bound until the kernel killed the whole container; the dump now marks
// the back-edge `<cycle>` and caps its depth, and `mem<=512` makes a
// regression die alone, as KILLED, which `xfail` cannot mask.
template <class T> struct Box {
  template <class U> friend class Box;
  T value;
};
Box<int> bi;
Box<char> bc;
Box<Box<int>> bb;
int get(Box<int> b) { return b.value; }
