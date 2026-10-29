//@ expect: ast
//@ oracle: clang
// [conv.prom]/5: a bit-field operand of a binary or unary operator
// promotes by its width (to int when int holds every value, to unsigned
// int at int's width, not at all when wider); an enumeration bit-field
// promotes as its type. Increment and decrement do not promote
// ([expr.pre.incr], [expr.post.incr]).
enum E { E0, E1 };
struct B {
  unsigned c : 9;
  int s : 7;
  unsigned w : 32;
  unsigned long ul : 40;
  bool b : 1;
  E e : 2;
};
void g(B *p) {
  (void)(p->c + 2);
  (void)(p->c + 2u);
  (void)(p->w + 1);
  (void)(p->ul + 1);
  (void)(p->b + p->b);
  (void)(p->e + 1);
  (void)-p->c;
  (void)~p->c;
  ++p->c;
  p->s--;
  p->w++;
}
