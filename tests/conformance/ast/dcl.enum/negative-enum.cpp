//@ expect: ast
enum Small { Neg = -1, Pos = 1 };
enum Big { Huge = 0x100000000LL };
enum UBig { Top = 0xFFFFFFFFu };
long width(Small s, Big b, UBig u) { return (long)s + (long)b + (long)u + s * 2 + u; }
