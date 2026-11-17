inline int f(int a) { return a + 0x10; }
int (*keep_b)(int) = f;
