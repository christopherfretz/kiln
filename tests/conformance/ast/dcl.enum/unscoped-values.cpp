//@ expect: ast
enum Color { Red, Green = 4, Blue, Neg = -2, After };
int pick(Color c) { return c == Blue ? After : Red; }
