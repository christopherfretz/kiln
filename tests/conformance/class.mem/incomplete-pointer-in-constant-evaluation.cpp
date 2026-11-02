//@ expect: compile-ok
//@ strict-diags
// [class.mem]/7, [basic.types.general]/5: a constant evaluation inside the
// member-specification of S that only names `S*` while S is incomplete
// must not fix S's layout; S is complete at `}` with all its members.
// (WP-267: the evaluator's type table kept S opaque, and sema's layout
// cache held the partial layout, until this was fixed.)
struct S;
consteval int probe(S* p) { return p ? 1 : 0; }
consteval int probe_ref(const S& s);
struct S {
  int a;
  static_assert(probe(nullptr) == 0);
  long b;
};
constexpr S s{1, 2};
static_assert(s.b == 2);
static_assert(sizeof(S) == 16);
static_assert(probe(const_cast<S*>(&s)) == 1);

struct T {
  struct U;
  static_assert(sizeof(U*) == 8);
  struct U { char c; double d; };
  static_assert(sizeof(U) == 16);
};
