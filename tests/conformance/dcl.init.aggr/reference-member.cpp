//@ expect: run
//@ strict-diags
// [dcl.init.aggr]/4.2, [dcl.init.ref]: an aggregate's reference member is
// bound to its initializer, not assigned through.
struct Ref { int& r; };
struct Pair { int& a; const int& b; int c; };

int main() {
    int i = 1, j = 2;
    Ref x{i};
    x.r = 5;
    if (i != 5) return 1;
    Pair p{j, i, 7};
    p.a = 3;
    if (j != 3 || p.b != 5 || p.c != 7) return 2;
    Pair q{i, 40 + 2, 0};  // the temporary is extended with q
    if (&q.a != &i || q.b != 42) return 3;
    return 0;
}
