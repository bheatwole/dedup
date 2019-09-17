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