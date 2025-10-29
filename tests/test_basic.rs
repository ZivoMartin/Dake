mod common;
use std::path::PathBuf;

use anyhow::Result;

use crate::common::cluster::{clean_cluster, setup_cluster};

const MAKEFILE: &'static str = "
#!ROOT_DEF NODE-1 = /project
#!ROOT_DEF NODE-2 = /project

main: main.o a.o b.o c.o
	$(CC) -o main main.o a.o b.o c.o

main.o: main.c
	$(CC) -c main.c

a.o[NODE-1]: a.c
	$(CC) -c a.c -o a.o

b.o[NODE-2]: b.c
	$(CC) -c b.c -o b.o

c.o[NODE-2]: c.c
	$(CC) -c c.c -o c.o
";

const MAIN: &'static str = r#"
#include <stdio.h>
int a(void);
int b(void);
int c(void);
int main() {
    printf("sum = %d\n", a() + b() + c());
    return 0;
}"#;

const A: &'static str = "int a(void) { return 1; }\n";
const B: &'static str = "int b(void) { return 2; }\n";
const C: &'static str = "int c(void) { return 3; }\n";

#[tokio::test(flavor = "multi_thread")]
async fn test_basic_build() -> Result<()> {
    let cluster = setup_cluster().await?;
    println!("Cluster ready: {:?}", cluster.nodes);

    let files = vec![
        (PathBuf::from("Makefile"), MAKEFILE.to_string()),
        (PathBuf::from("a.c"), A.to_string()),
        (PathBuf::from("b.c"), B.to_string()),
        (PathBuf::from("c.c"), C.to_string()),
        (PathBuf::from("main.c"), MAIN.to_string()),
    ];
    let dest_path = PathBuf::from("test_basic");
    cluster.push_files(files, &dest_path).await?;

    cluster
        .start_dake(dest_path, &cluster.nodes[0], PathBuf::from("test_basic"))
        .await?;

    clean_cluster().await?;
    Ok(())
}
