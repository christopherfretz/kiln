//@ expect: pp
//@ oracle: none
// Two tokens glued into one in the golden: a token-wise mismatch.
#define P(a, b) a b
P(+, +) x;
