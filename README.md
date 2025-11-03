# Dake - Distributed Makefile System

Dake is a distributed build system that extends traditional Makefiles to work seamlessly across multiple machines. It allows you to assign build targets to different hosts, automatically handle dependency fetching, and run remote builds with minimal overhead. The system is designed to be **fully compatible with standard Makefile syntax** while enabling transparent distribution.

---

## Introduction

In classic Make-based workflows, all build commands execute on a single host, even if you have multiple machines available. Dake changes that: it introduces lightweight daemons that communicate over the network and cooperatively build your project.

Each daemon can compile a subset of targets, share built artifacts, and synchronize dependencies with others. The developer writes a regular Makefile, annotating only which targets belong to which node. Everything else, distribution, fetching, caching, linking, is handled automatically.

---

## Tutorial: Using Dake Step by Step

### 1. Build Dake on Each Host

Each machine participating in the distributed build must have Dake installed.

First, clone the repository and build it in release mode:

```bash
git clone https://github.com/yourusername/dake.git
cd dake
cargo build --release
```

The executable will be generated at:

```
target/release/dake
```

Copy it to a directory in your PATH:

```bash
sudo cp target/release/dake /usr/local/bin/
```

You can check that it is installed correctly:

```bash
dake --version
```

---

### 2. Launch the Daemon on Each Host

On every host that will participate in the distributed build, start the Dake daemon:

```bash
dake daemon
```

By default, the daemon listens for incoming connections on port `1808`. You can launch it manually in a terminal or start it in the background:

```bash
nohup dake daemon > dake.log 2>&1 &
```

Repeat this step on each machine that will be part of the distributed network.

---

### 3. Write a Distributed Makefile

A distributed Makefile looks almost identical to a standard one. You only need to define which nodes correspond to which working directories, and assign targets to them.

#### Example

```makefile
#!ROOT_DEF 172.0.0.2  = /project
#!ROOT_DEF 172.0.0.3 = /project

main: main.o a.o b.o
	$(CC) -o main main.o a.o b.o

main.o: main.c
	$(CC) -c main.c -o main.o

a.o[172.0.0.2]: a.c
	$(CC) -c a.c -o a.o

b.o[172.0.0.3]: b.c
	$(CC) -c b.c -o b.o
```

#### Explanation

* `#!ROOT_DEF 172.0.0.2 = /project` defines that `172.0.0.2` corresponds to the daemon working in `/project` on its host.
* `a.o[172.0.0.2]` means that target `a.o` will be built remotely by the daemon running on `172.0.0.2`.
* You can use a DNS name to specify the target host, and optionally append |path to define the project directory directly on that host.
* Dependencies are automatically fetched when required, and all commands use standard Makefile syntax.

---

### 4. Prepare the Project Files

Copy all project files (the Makefile and sources) to the working directory of each node, as specified in your root definitions.

For the example above, you would place these files on both hosts under `/project`:

```bash
/project/
├── Makefile
├── main.c
├── a.c
└── b.c
```

---

### 5. Run the Distributed Build

Once the daemons are running and all nodes contain the project files, start the build from **any** node:

```bash
dake
```

Dake will:

1. Parse the Makefile.
2. Resolve all node definitions.
3. Dispatch build commands to the appropriate hosts.
4. Fetch remote build artifacts when needed.
5. Link the final binary locally.

---

### 6. Verify the Result

After the build completes, you can run the resulting executable as usual:

```bash
./main
```

Expected output:

```
sum = 3
```

---

### 7. Stop the Daemons

Once you are done, stop all daemons manually:

```bash
pkill dake
```

Or, if you have set them up as services:

```bash
sudo systemctl stop dake
```

---

## Summary

Dake enables you to:

* Write **standard Makefiles** with minimal extensions.
* Build **across multiple machines** automatically.
* Reuse your existing toolchains and compilers.
* Keep the setup simple and reproducible.

All you need is to:

1. Build Dake on each host.
2. Launch `dake daemon` everywhere.
3. Write a Makefile with node annotations.
4. Run `dake` from any node.

And that’s it, your builds are now distributed.
