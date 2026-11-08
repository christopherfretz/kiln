// [basic.scope.pdecl]/7, [namespace.memdef]/3: `friend class X;` declares
// X in the innermost enclosing namespace, hidden from ordinary lookup; a
// later member `class X *p;` in the same class finds no visible X and
// declares X in that namespace, redeclaring the hidden class (clang
// `ActOnTag`'s redeclaration lookup), so the namespace-scope definition
// is the befriended class and X is visible after the member (z3's
// `smt::context` befriending `smt::parallel`). WP-799.
//@ strict-diags
namespace smt {
class context {
  friend class parallel;
  int m = 1;
  class parallel *m_par = nullptr;
};
parallel *visible_after_member = nullptr;
class parallel {
  int v;

public:
  parallel(context &c) : v(c.m) {}
};

struct holder {
  friend struct item;
  int secret = 2;
  class item *first = nullptr;
};
struct item {
  int get(holder &h) { return h.secret; }
};

template <class T> class box {
  friend class opener;
  T inner{};
  class opener *op = nullptr;
};
class opener {
public:
  int peek(box<int> &b) { return b.inner; }
};
} // namespace smt

int main() {
  smt::context c;
  smt::holder h;
  smt::box<int> b;
  (void)smt::parallel(c);
  return smt::item().get(h) + smt::opener().peek(b);
}
