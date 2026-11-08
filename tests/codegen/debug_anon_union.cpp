// Debug info (WP-814): an anonymous union member.
//@ args: -g
struct U {
  int tag;
  union {
    int i;
    float fl;
  };
};
int get(U u) { return u.tag ? u.i : (int)u.fl; }
