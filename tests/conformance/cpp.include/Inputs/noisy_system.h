// A header that warns unless it is a system header: every warning here is
// suppressed when it is found through -isystem (and -Werror does not
// resurrect it), and reported under -Wsystem-headers.
#pragma message "noisy system header"
#define NOISY 1
#define NOISY 2
int noisy_multichar = 'ab';
[[gnu::not_an_attribute]] int noisy_attr;
