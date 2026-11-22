// The written-reference table (spec/ast.md §18), a small whole program
// reduced from real code: a call found by argument-dependent lookup, a
// member of a parameter, a dependent call in a function template and the
// call of its specialization, qualifiers before type names and before an
// out-of-line definition's name. Stage 1 records expression, member and
// qualifier names; the type, template, base and using-declaration names
// here are later stages and have no entries yet.
//@ written-refs: example.refs
namespace store {
namespace disk {
struct Page {
  int used;
};
void sync(Page&);
template <class T> struct Slot {
  T item;
};
} // namespace disk
} // namespace store

namespace app {
using store::disk::Page;
struct Cache : store::disk::Slot<Page> {
  void flush(Page& p) {
    sync(p);
    p.used = 0;
  }
};
template <class T> void every(T& t) { sync(t); }
void drain(store::disk::Page& p) { every(p); }
} // namespace app

void store::disk::sync(Page&) {}
