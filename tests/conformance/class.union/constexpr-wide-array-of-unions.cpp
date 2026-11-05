// [class.union.general]/6, [dcl.array]/6, [expr.const]/10.9 (WP-642): an
// array of structs, each wide enough for the Oven's offset index (16 or
// more fields) and holding unions at several offsets, is stored element by
// element in a loop. Each element's unions switch their active member
// independently; a read of an inactive member in one element is diagnosed
// while the same member in a neighbouring element is active.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 51:15 error "static assertion expression is not an integral constant expression"
//@ diag: 49:42 note "read of member 'i' of union with active member 'f' is not allowed in a constant expression"
//@ diag: 51:15 note "in call to 'read_neighbour()'"
union U {
  int i;
  float f;
};
struct Row {
  U head;
  int c0, c1, c2, c3, c4, c5, c6, c7, c8, c9, c10, c11, c12, c13, c14;
  U mid;
  int c15;
  U tail;
};

constexpr int fill(int n) {
  Row rows[8]{};
  for (int k = 0; k < n; ++k) {
    rows[k].head.i = k;
    rows[k].mid.f = static_cast<float>(k);
    rows[k].tail.i = 2 * k;
    rows[k].c15 = 1;
    if (k % 2)
      rows[k].tail.f = static_cast<float>(3 * k);
  }
  int s = 0;
  for (int k = 0; k < n; ++k) {
    s += rows[k].head.i + static_cast<int>(rows[k].mid.f) + rows[k].c15;
    s += k % 2 ? static_cast<int>(rows[k].tail.f) : rows[k].tail.i;
  }
  return s;
}
// head 28 + mid 28 + c15 8 + tail (2*(0+2+4+6) + 3*(1+3+5+7)) 72
static_assert(fill(8) == 136);

constexpr int read_neighbour() {
  Row rows[4]{};
  for (int k = 0; k < 4; ++k)
    rows[k].mid.i = k;
  rows[2].mid.f = 1.5f;
  return rows[1].mid.i + rows[3].mid.i + rows[2].mid.i;
}
static_assert(read_neighbour() == 4);
