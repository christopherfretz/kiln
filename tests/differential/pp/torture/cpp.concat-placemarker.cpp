//@ expect: pp
// [cpp.concat]: an empty argument next to ## is a placemarker; placemarkers
// vanish, and pasting with one yields the other operand unchanged.
#define t(x, y, z) x ## y ## z
int j[] = { t(1,2,3), t(,4,5), t(6,,7), t(8,9,),
            t(10,,), t(,11,), t(,,12), t(,,) };

#define CAT(a, b) a ## b
#define CAT3(a, b, c) a ## b ## c
CAT(,) | CAT(x,) | CAT(,y) | CAT3(,,) | CAT3(a,,) | CAT3(,b,) | CAT3(,,c)

// `# ## #` pastes two `#` tokens into `##`, which is then an ordinary token.
#define hash_hash # ## #
#define mkstr(a) # a
#define in_between(a) mkstr(a)
#define join(c, d) in_between(c hash_hash d)
char p[] = join(x, y);

// A placemarker does not suppress expansion of what is left.
#define ONE 1
#define PASTE_EMPTY(a, b) a ## b
PASTE_EMPTY(ONE,) PASTE_EMPTY(,ONE)
