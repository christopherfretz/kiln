//@ expect: pp
// Only parentheses group arguments: commas inside brackets and braces
// separate arguments, commas inside parentheses do not.
#define N(...) <__VA_ARGS__>
#define TWO(a, b) {a|b}
#define ONE(a) {a}
TWO([1, 2]) TWO({1, 2}) ONE((1, 2)) ONE(((1), (2)))
TWO(,) TWO( , ) ONE() ONE( )
TWO(a<b, c>d) N(a<b, c>d)
TWO("a,b", 'c') TWO(f(a, b), g(c))
#define THREE(a, b, c) a-b-c
THREE(,,) THREE(1,,3) THREE((,),(,),(,))
N(()) N(()()) N(,,,)
