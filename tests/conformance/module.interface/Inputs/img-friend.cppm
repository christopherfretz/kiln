// Friends of classes for the declaration image (spec/modules.md §11.11
// "Friends", WP-1292).
export module imgfr;
export namespace fr {
int peek(const struct Box &b);
struct Box {
  Box(int v) : v(v) {}
  // Hidden friends: found only by argument-dependent lookup.
  friend bool operator==(const Box &a, const Box &b) { return a.v == b.v; }
  friend int twice(const Box &b) { return b.v * 2; }
  // A friend redeclaring a function declared before the class.
  friend int peek(const Box &b);
  // Befriended classes: one introduced by the friend declaration, one
  // declared by an elaborated specifier.
  friend struct Helper;
  friend class Other;

private:
  int v;
};
int peek(const Box &b) { return b.v + 100; }
struct Helper {
  static int get(const Box &b) { return b.v + 1; }
};
class Other {
public:
  static int get(const Box &b) { return b.v - 1; }
};
struct Ord {
  int k;
  friend constexpr bool operator<(Ord a, Ord b) { return a.k < b.k; }
  friend constexpr bool operator==(Ord a, Ord b) { return a.k == b.k; }
};
} // namespace fr
