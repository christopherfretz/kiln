// Plain char is unsigned on AArch64 and signed on x86_64.
int widen(char c) { return c; }
int cmp(char c) { return c < 0; }
int sign(char c) { return widen(c) + cmp(c) + (char)200; }
