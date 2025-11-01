use std::path::PathBuf;

const MAKEFILE: &'static str = "
#!ROOT_DEF NODE-1 = /test_fetch_chain
#!ROOT_DEF NODE-2 = /test_fetch_chain
#!ROOT_DEF NODE-3 = /test_fetch_chain

main: main.o a.o b.o c.o
	$(CC) -o main main.o a.o b.o c.o
main.o: main.c a.o
	$(CC) -c main.c -o main.o

a.o[NODE-1]: a.c b.o
	$(CC) -c a.c -o a.o

b.o[NODE-2]: b.c c.o
	$(CC) -c b.c -o b.o

c.o[NODE-3]: c.c
	$(CC) -c c.c -o c.o";

const MAIN: &'static str = r#"
#include <stdio.h>
int a(void);
int main() {
    printf("sum = %d\n", a());
    return 0;
}"#;

const A: &'static str = "
int b(void);
int a(void) { return 1 + b(); }\n";

const B: &'static str = "
int c(void);
int b(void) { return 2 + c(); }\n";

const C: &'static str = "int c(void) { return 3; }\n";

pub fn test_fetch_chain_build() -> (Vec<(PathBuf, String)>, PathBuf, String) {
    (
        vec![
            (PathBuf::from("Makefile"), MAKEFILE.to_string()),
            (PathBuf::from("a.c"), A.to_string()),
            (PathBuf::from("b.c"), B.to_string()),
            (PathBuf::from("c.c"), C.to_string()),
            (PathBuf::from("main.c"), MAIN.to_string()),
        ],
        PathBuf::from("/test_fetch_chain"),
        "sum = 6\n".to_string(),
    )
}
