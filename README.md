# Rabin Variable-Sized Chunk Deduplication
This library using the Rabin fingerprint method to find chunks of a data stream that are duplicates. The primary use
cases are to reduce the amount of data transferred over a network, or to reduce the size of backups.

## Deduplication
Deduplication is a lossless form of compression that identifies each unique chunk of data and only transmits any
particular chunk once. There are three main methods of deduplication (outlined below)

### File Level Deduplication
For file-based data streams, the file boundary makes a convenient place to break the stream into chunks. A simple SHA256
hash of the file contents will tell you (with a reasonable degree of certainty) that the file is unique. The issue with
this method is that unless you are primarily backing up many files that are identical, you will not save much space.
Backing up a large number of desktop machines that use similar versions of a particular OS may see some savings using
this method. This method is also effective if the same data is backed up periodically to full backups.

### Fixed-Size Deduplication
With fixed-size deduplication, the data stream is broken into many chunks of exactly the same size (4KiB is convenient
as it is the page size on many machines). This will deduplicate all the same files as File Level Deduplication, but will
probably also find some chunks of files that are repeated (for example, some file formats have unused space that is set
to all zeros). The issue with fixed-size deduplication is that the addition or removal of a single byte from the middle
of a file will make all fixed size chunks that come after it have slightly different contents. Thus, in practice, it
does not have much significant benefit over file-level deduplication.

### Variable-Size Deduplication
Instead of breaking the data stream into fixed-size chunks, variable-sized deduplication looks for a pattern of bits
within the data stream and breaks it there. (In practice a minimum and maximum chunk size are also enforced). The
pattern is based on a hash of a sliding window of a few bytes of data in the stream. The hash gives a repeatable
algorithm that also has the effect of randomizing the data so that we get an even distribution of cut-points.

## Fixed vs Variable
The test_chunks application in this repository implements both fixed and variable chunking given a filesystem directory.
You can run the application in both modes to see how the two methods compare.

## Compiling and Running test_chunks
The goal of this application is to scan a very large directory and determine how much memory would be needed to store
the hash of every chunk. It also determines how many unique chunks are in the directory and if there would be any 
collisions using a shortened hash to save on storage.

### Compiling with Rust
If you have Rust installed on your machine you can run:
```
git clone https://github.com/bheatwole/dedup.git
cd dedup/test_chunks
cargo run --release -- -d /some/directory/with/lots/of/data -o /an/empty/directory/to/hold/output/files -m 500M
```

### Compiling with Docker
```
git clone https://github.com/bheatwole/dedup.git
cd dedup
docker build -t dedup_test_chunks .
docker run --rm -v /some/directory/with/lots/of/data:/data ecr.cwi.name/dedup_test_chunks -d /data -o /output -m 500m
```

### test_chunks Arguments
- -d, --directory: The directory in which to start scanning all files.
- -o, --output: The directory in which to store the output files of the application
- -m, --memory: The number of bytes to use for storing hashes (i.e. 500k, 100m, 1G, etc). When this is exceeded, a file is written to /output and the hash table cleared for more data.
- -f, --fixed: If set, a fixed size chunk of 4096 bytes will be used instead of the variable-size algorithm. Used to test the difference in performance and chunk quality.