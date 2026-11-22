// Per-specialization entries (spec/ast.md §18.9.2): a variable template's
// initializer is instantiated with its specialization ([temp.inst]/7):
// `spec=` names the variable template specialization. An alias template
// has no specialization declaration ([temp.alias]/2: its template-id is
// the substituted type), so its lines name the alias template, one per
// distinct resolution.
//@ written-refs: inst-var-template.refs
struct Meters {
  static constexpr int scale = 100;
  using unit = long;
};
struct Feet {
  static constexpr int scale = 30;
  using unit = short;
};

template <class T> constexpr int scale_of = T::scale;
template <class T> using unit_of = typename T::unit;

static_assert(scale_of<Meters> == 100);
static_assert(scale_of<Feet> == 30);
unit_of<Meters> m = 1;
unit_of<Feet> f = 2;
