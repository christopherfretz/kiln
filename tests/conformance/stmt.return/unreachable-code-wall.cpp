// [stmt.return]/1 (WP-234): -Wall does not enable -Wunreachable-code.
//@ args: -Wall
//@ strict-diags
int g(int);
void e1() { return; g(1); }
