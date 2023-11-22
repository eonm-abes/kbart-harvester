<div align="center">

# KBART Harverster

</div>

## Installation

````sh
git clone && cd kbart-harvester
cargo install -p .
````

## Usage

````sh
kbart-harverster -i urls_list.txt -o kbarts

cat urls_list.txt | kbart-harverster -o kbarts
````

**Parameters :**

* `--input` `-i`: specifies the path to the input file. The file must contain one URL per line. This parameter is optional if URLs are piped through STDIN.
* `--output-dir` `-o` : set the output directory.
* `--nocheck` `-n`: don't check the validity of the kbart file (see [below]()).
* `--workers` `-w` : set the number of workers used to download the files. Files will be downloaded in parallel.

## KBART validity check

By default, KBART Harverster checks the validity of a KBART file before fully downloading it. The check consists of comparing the first bytes of the KBART file with a valid header. Only the first columns of the header are compared beacause : some editors add their own columns at the end of the file) ; last headers have subtle differences between KBART versions.

## File naming

Files are automatically named based on the last path in the URL.The name is sanitised for security reasons.If the URL doesn't have a path, an error is returned.

**⚠️  If the file already exists, it's contents will be overridden.**
