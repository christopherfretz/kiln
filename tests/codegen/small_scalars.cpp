// bool / char / short / unsigned parameters and returns: signext and
// zeroext attributes, and integer promotions.
bool flip(bool b) { return !b; }
char cnext(char c) { return c + 1; }
signed char scneg(signed char c) { return -c; }
unsigned char ucinc(unsigned char c) { return c + 1; }
short sh(short s, unsigned short u) { return s + u; }
unsigned long ul(unsigned int u) { return u; }
long sl(int i) { return i; }
int scal(bool b, char c, short s) { return flip(b) + cnext(c) + scneg(c) + ucinc(c) + sh(s, s) + (int)ul(3) + (int)sl(-4); }
