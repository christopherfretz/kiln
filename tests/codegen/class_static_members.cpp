// Static data members: declarations (external), out-of-line definitions,
// inline and constexpr members, and odr-used in-class-initialized const
// members (available_externally).
struct P { int x, y; };
struct S {
  static int count;
  static int defined;
  static const int k = 3;
  static const int k2 = 9;
  static inline int i = 4;
  static constexpr long c = 5;
  static constexpr P p{1, 2};
  static int bump() { return ++defined; }
};
int S::defined = 7;
const int S::k2;

const int* addr_k() { return &S::k; }
const int* addr_k2() { return &S::k2; }
int sum() { return S::count + S::k + S::i + (int)S::c + S::p.y + S::bump(); }
const P* addr_p() { return &S::p; }
