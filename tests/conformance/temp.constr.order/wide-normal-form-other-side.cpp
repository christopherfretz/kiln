// [temp.constr.order]/1: subsumption between normal forms whose DNF or
// CNF has more clauses than a conversion may build is still decided from
// the other form: a conjunction of six seven-way disjunctions (7^6 DNF
// clauses) subsumes one of its own disjunctions, and an 80-operand
// disjunction is normalized as a whole and subsumed by a conjunction of
// two of its operands (WP-958).
//@ std: c++20
//@ expect: compile-ok
template <typename T> concept C0 = true;
template <typename T> concept C1 = true;
template <typename T> concept C2 = true;
template <typename T> concept C3 = true;
template <typename T> concept C4 = true;
template <typename T> concept C5 = true;
template <typename T> concept C6 = true;
template <typename T> concept C7 = true;
template <typename T> concept C8 = true;
template <typename T> concept C9 = true;
template <typename T> concept C10 = true;
template <typename T> concept C11 = true;
template <typename T> concept C12 = true;
template <typename T> concept C13 = true;
template <typename T> concept C14 = true;
template <typename T> concept C15 = true;
template <typename T> concept C16 = true;
template <typename T> concept C17 = true;
template <typename T> concept C18 = true;
template <typename T> concept C19 = true;
template <typename T> concept C20 = true;
template <typename T> concept C21 = true;
template <typename T> concept C22 = true;
template <typename T> concept C23 = true;
template <typename T> concept C24 = true;
template <typename T> concept C25 = true;
template <typename T> concept C26 = true;
template <typename T> concept C27 = true;
template <typename T> concept C28 = true;
template <typename T> concept C29 = true;
template <typename T> concept C30 = true;
template <typename T> concept C31 = true;
template <typename T> concept C32 = true;
template <typename T> concept C33 = true;
template <typename T> concept C34 = true;
template <typename T> concept C35 = true;
template <typename T> concept C36 = true;
template <typename T> concept C37 = true;
template <typename T> concept C38 = true;
template <typename T> concept C39 = true;
template <typename T> concept C40 = true;
template <typename T> concept C41 = true;
template <typename T> concept C42 = true;
template <typename T> concept C43 = true;
template <typename T> concept C44 = true;
template <typename T> concept C45 = true;
template <typename T> concept C46 = true;
template <typename T> concept C47 = true;
template <typename T> concept C48 = true;
template <typename T> concept C49 = true;
template <typename T> concept C50 = true;
template <typename T> concept C51 = true;
template <typename T> concept C52 = true;
template <typename T> concept C53 = true;
template <typename T> concept C54 = true;
template <typename T> concept C55 = true;
template <typename T> concept C56 = true;
template <typename T> concept C57 = true;
template <typename T> concept C58 = true;
template <typename T> concept C59 = true;
template <typename T> concept C60 = true;
template <typename T> concept C61 = true;
template <typename T> concept C62 = true;
template <typename T> concept C63 = true;
template <typename T> concept C64 = true;
template <typename T> concept C65 = true;
template <typename T> concept C66 = true;
template <typename T> concept C67 = true;
template <typename T> concept C68 = true;
template <typename T> concept C69 = true;
template <typename T> concept C70 = true;
template <typename T> concept C71 = true;
template <typename T> concept C72 = true;
template <typename T> concept C73 = true;
template <typename T> concept C74 = true;
template <typename T> concept C75 = true;
template <typename T> concept C76 = true;
template <typename T> concept C77 = true;
template <typename T> concept C78 = true;
template <typename T> concept C79 = true;

template <typename T>
concept Wide =
    C0<T> || C1<T> || C2<T> || C3<T> || C4<T> || C5<T> || C6<T> || C7<T> ||
    C8<T> || C9<T> || C10<T> || C11<T> || C12<T> || C13<T> || C14<T> || C15<T> ||
    C16<T> || C17<T> || C18<T> || C19<T> || C20<T> || C21<T> || C22<T> || C23<T> ||
    C24<T> || C25<T> || C26<T> || C27<T> || C28<T> || C29<T> || C30<T> || C31<T> ||
    C32<T> || C33<T> || C34<T> || C35<T> || C36<T> || C37<T> || C38<T> || C39<T> ||
    C40<T> || C41<T> || C42<T> || C43<T> || C44<T> || C45<T> || C46<T> || C47<T> ||
    C48<T> || C49<T> || C50<T> || C51<T> || C52<T> || C53<T> || C54<T> || C55<T> ||
    C56<T> || C57<T> || C58<T> || C59<T> || C60<T> || C61<T> || C62<T> || C63<T> ||
    C64<T> || C65<T> || C66<T> || C67<T> || C68<T> || C69<T> || C70<T> || C71<T> ||
    C72<T> || C73<T> || C74<T> || C75<T> || C76<T> || C77<T> || C78<T> || C79<T>;

template <typename T>
concept Narrow = C0<T> && C1<T>;

constexpr int f(Wide auto) { return 1; }
constexpr int f(Narrow auto) { return 2; }
static_assert(f(0) == 2);

template <typename T>
concept Cnf6 =
    (C0<T> || C1<T> || C2<T> || C3<T> || C4<T> || C5<T> || C6<T>) &&
    (C7<T> || C8<T> || C9<T> || C10<T> || C11<T> || C12<T> || C13<T>) &&
    (C14<T> || C15<T> || C16<T> || C17<T> || C18<T> || C19<T> || C20<T>) &&
    (C21<T> || C22<T> || C23<T> || C24<T> || C25<T> || C26<T> || C27<T>) &&
    (C28<T> || C29<T> || C30<T> || C31<T> || C32<T> || C33<T> || C34<T>) &&
    (C35<T> || C36<T> || C37<T> || C38<T> || C39<T> || C40<T> || C41<T>);

template <typename T>
concept OneClause = (C21<T> || C22<T> || C23<T> || C24<T> || C25<T> || C26<T> || C27<T>);

constexpr int g(OneClause auto) { return 1; }
constexpr int g(Cnf6 auto) { return 2; }
static_assert(g(0) == 2);
