// [temp.deduct.conv]/1, [over.match.copy]/1.2, [over.match.conv]/1,
// [temp.inst]/1: inside an instantiation of an enclosing function or class
// template (and of a lambda in one), an implicit conversion through a
// conversion function template deduces the member template's own `T` from
// the target type. The member template's parameter (depth 0 of a
// non-template class, depth 1 of a class template) is never the enclosing
// instantiation's argument of the same position (WP-563).
//@ expect: run
//@ stdout: 7 7 7 7 7
//@ stdout: 7 7 7 7 7
//@ stdout: 11 12 13 14
//@ stdout: 3 5
#include <cstdio>
#include <type_traits>

struct handle { int id = 0; };
struct record : handle {};

struct getter {
  int seed;
  template <class U> operator U() const { U u; u.id = seed; return u; }
};
template <class Tag> struct tagged_getter {
  int seed;
  template <class U> operator U() const { U u; u.id = seed + int(sizeof(Tag)) - int(sizeof(Tag)); return u; }
};
struct ns { getter fetch(int n) { return {n}; } };

enum class colour { red };

// The shape found in real code: constrained `const &` and `&&` overloads.
struct picky {
  int seed;
  template <class U, std::enable_if_t<std::is_base_of_v<handle, U>, int> = 0>
  operator U() const & { U u; u.id = seed; return u; }
  template <class U, std::enable_if_t<std::is_base_of_v<handle, U>, int> = 0>
  operator U() && { U u; u.id = seed + 10; return u; }
};
picky make_picky(int n) { return {n}; }

template <class Self> int copy_init(ns& s) { handle h = s.fetch(7); return h.id; }
template <class Self> int from_lvalue(ns& s) { getter g = s.fetch(7); handle h = g; return h.id; }
template <class Self> int direct_init(ns& s) { getter g = s.fetch(7); handle h(g); return h.id; }
template <class Self> int casts(ns& s) {
  getter g = s.fetch(7);
  return static_cast<handle>(g).id == handle(g).id ? handle(g).id : -1;
}
template <class Self> handle returned(ns& s) { getter g = s.fetch(7); return g; }
template <class Self> struct holder {
  static int via_member(ns& s) { getter g = s.fetch(7); handle h = g; return h.id; }
  static int via_lambda(ns& s) {
    getter g = s.fetch(7);
    auto l = [&] { handle h = g; return h.id; };
    return l();
  }
};
template <class Self> int nested_lambda(ns& s) {
  getter g = s.fetch(7);
  return [&](auto) { handle h = g; return h.id; }(0);
}
template <class Self> int class_member_template(int n) {
  tagged_getter<Self> g{n};
  handle h = g;
  return h.id;
}
template <class Self> int picky_lvalue() { picky p{11}; handle h = p; return h.id; }
template <class Self> int picky_rvalue() { record r = make_picky(2); return r.id; }
template <class Self> int picky_derived() { picky p{13}; record r(p); return r.id; }
template <class Self> int picky_return() { picky p{14}; return [&]() -> handle { return p; }().id; }
template <class Self> int to_int() { getter g{3}; struct box { int id; } b = g; return b.id; }

int main() {
  ns s;
  std::printf("%d %d %d %d %d\n", copy_init<colour>(s), copy_init<int>(s), from_lvalue<int>(s),
              direct_init<colour>(s), casts<char>(s));
  std::printf("%d %d %d %d %d\n", returned<int>(s).id, holder<int>::via_member(s),
              holder<colour>::via_lambda(s), nested_lambda<double>(s),
              class_member_template<long>(7));
  std::printf("%d %d %d %d\n", picky_lvalue<colour>(), picky_rvalue<int>(), picky_derived<int>(),
              picky_return<colour>());
  std::printf("%d %d\n", to_int<int>(), class_member_template<char>(5));
  return 0;
}
