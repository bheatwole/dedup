use std::collections;
use std::fs;
use std::io;
use std::path;
use std::time;

use bincode;
use clap;
use regex;
use serde_derive::{Deserialize, Serialize};

pub const KEY_LEN: usize = 18;
pub const ENTRY_LEN: usize = 24;
// These constants were calculated based on information provided in http://www.hpl.hp.com/techreports/2005/HPL-2005-30R1.pdf
pub const MIN_CHUNK_SIZE: usize = 1856;
pub const MAX_CHUNK_SIZE: usize = 11300;

// RESULTS OF TESTING
// 1) Even with only 18 bytes per key, there are just too many keys to hold in memory for small clusters. It's very
//    close though, so a change in the amount of memory typically available or a decrease in the number of chunks
//    would warrent a re-evaluation of that assumption.
// 2) The variable-sized chunking really does have an effect. Both on the number of chunks in a single backup, but
//    especially as files are edited for subsequent backups.

fn main() {
    let started = time::Instant::now();
    let matches = clap::App::new("Test Backup Chunks")
                            .version("1.0")
                            .author("Benjamin Heatwole <bheatwole@cwi-va.com")
                            .about("Tests the requirements for backup chunking on a particular directory")
                            .arg(clap::Arg::with_name("directory")
                                           .short("d")
                                           .long("directory")
                                           .value_name("DIR")
                                           .help("The directory to scan for chunks.")
                                           .takes_value(true)
                                           .required(true))
                            .arg(clap::Arg::with_name("output")
                                           .short("o")
                                           .long("output")
                                           .value_name("DIR")
                                           .help("The directory to store the output.")
                                           .takes_value(true)
                                           .required(true))
                            .arg(clap::Arg::with_name("memory")
                                           .short("m")
                                           .long("memory")
                                           .value_name("BYTES")
                                           .help("The amount of memory to use for sorting. Use 'K', 'M' and 'G' abbreviations. I.E. 100M.")
                                           .takes_value(true)
                                           .required(true))
                            .arg(clap::Arg::with_name("fixed")
                                           .short("f")
                                           .help("If set, a fixed size chunk of 4096 will be used instead of the variable sized chunks"))
                            .get_matches();

    // When chunking large directories, we can run out of memory to store all the chunk hashes. Determine how much the
    // user is willing to set aside and then use that as the max for the chunk btree. The actual usage will probably be
    // close to double that because the hash tends to insert into the tree pretty balanced which leaves plenty of nodes
    // with about half the space empty.
    let memory_usage = parse_memory_usage(matches.value_of("memory").unwrap());
    let btree_max_entries = ((memory_usage as usize / 10) * 8) / ENTRY_LEN;
    let mut memtree = collections::BTreeMap::new();
    let mut statistics = Statistics {
        unique_chunks: 0,
        duplicates: 0,
        unique_chunk_bytes: 0,
        duplicate_chunk_bytes: 0,
        collisions: 0,
    };
    let mut next_mem_id: usize = 0;

    // Confirm the output directory exists
    let out_dir = path::Path::new(matches.value_of("output").unwrap());
    if !out_dir.is_dir() {
        println!(
            "ERROR: the output directory '{:?}' does not exist or is a file",
            out_dir
        );
        return;
    }

    // Create the chunk hasher
    use rabin::ExtendableHashExt;
    use sha3::Digest;
    let mut hasher = sha3::Sha3_256::new();

    // Iterate through all the directories
    visit_dirs(
        path::Path::new(matches.value_of("directory").unwrap()),
        &mut |e| {
            // Chunk each file using either the variable-sized or fixed-size chunking algorithm
            chunk_file(&e.path(), matches.is_present("fixed"), &mut |c| {
                let key = hasher.hash_chunk_144(c);
                let check = sha2_check(c);

                let data = EntryData {
                    check: check,
                    size: c.len() as u16,
                };

                // Check to see if we already know about this chunk
                match memtree.insert(key, data) {
                    None => {
                        // Unique chunk, never seen before
                        statistics.unique_chunks += 1;
                        statistics.unique_chunk_bytes += c.len() as u64;
                    }
                    Some(old_data) => {
                        if old_data == data {
                            // The size of the data and both the SHA2 and SHA3 hashes match for the chunk, so the odds of it
                            // not being a perfect match are statistically miniscule.
                            statistics.duplicates += 1;
                            statistics.duplicate_chunk_bytes += c.len() as u64;
                        } else {
                            // COLLISION!!! Something didn't match, so the partial SHA3 hash we used as an ID is no good. We
                            // probably just need to increase the bits from 144
                            statistics.collisions += 1;
                        }
                    }
                };

                // If we have more entries in the memtree than we're supposed to, write the whole memtree to disk and
                // clear it for another round.
                if memtree.len() >= btree_max_entries {
                    write_memtree_file(out_dir.join(format!("mem_{}", next_mem_id)), &mut memtree);
                    next_mem_id += 1;
                }
            });
        },
    );

    // Write the last file
    if memtree.len() > 0 {
        write_memtree_file(out_dir.join(format!("mem_{}", next_mem_id)), &mut memtree);
        next_mem_id += 1;
    }

    // === Sorting Algorithm ===
    // The keys will be inserted into an in-memory sorted array until the sorting memory buffer is full. It will then
    // write out that chunk of sorted data to a temp file and start with a new empty buffer.
    //
    // When chunking is complete, the sorted temp files will be merged into a single file and the calculations on
    // compression level, chunk size and collisions will be performed.
    //
    // When 'merging' we don't actually care about the contents except to see if there are duplicates and/or collisions
    let mut merge_files = vec![];
    let mut merge_data: Vec<Option<Entry>> = vec![];
    for i in 0..next_mem_id {
        merge_files.push(io::BufReader::new(
            fs::File::open(out_dir.join(format!("mem_{}", i))).unwrap(),
        ));
        merge_data.push(Some(
            bincode::deserialize_from(merge_files.get_mut(i).unwrap()).unwrap(),
        ));
    }

    loop {
        let mut smallest_entry: Option<Entry> = None;
        let mut smallest_index = 0;

        // First find the smallest key in sorted order
        for i in 0..next_mem_id {
            let test_entry = merge_data.get(i).unwrap();
            match (smallest_entry, test_entry) {
                (_, None) => {}
                (None, Some(e)) => {
                    let copy = *e;
                    smallest_entry = Some(copy);
                    smallest_index = i;
                }
                (Some(left), Some(right)) => {
                    if right.key < left.key {
                        let copy = *right;
                        smallest_entry = Some(copy);
                        smallest_index = i;
                    }
                }
            }
        }

        // If there is no smallest, then we're totally done!
        if None == smallest_entry {
            break;
        }

        // Starting with the entry we found, check all remaining entries for duplicates and grab the next data element
        // from their file
        for i in smallest_index..next_mem_id {
            if i == smallest_index {
                // The first index is the one we found. It's not a duplicate, but it was also recorded earlier, so just
                // update the data
                merge_data[i] = match bincode::deserialize_from(merge_files.get_mut(i).unwrap()) {
                    Ok(e) => Some(e),
                    _ => None,
                }
            } else {
                let current_entry = merge_data.get(i).unwrap();
                match (smallest_entry, current_entry) {
                    (_, None) => {
                        // The file at this index is all done
                    }
                    (None, Some(_)) => {
                        // not possible because we check for None above
                    }
                    (Some(smallest), Some(borrowed)) => {
                        let current = *borrowed;
                        if current.key == smallest.key {
                            // Keys are duplicate. Check for collision
                            statistics.unique_chunks -= 1;
                            statistics.unique_chunk_bytes -= current.size as u64;
                            if current != smallest {
                                statistics.collisions += 1;
                            } else {
                                statistics.duplicates += 1;
                                statistics.duplicate_chunk_bytes += current.size as u64;
                            }

                            // Need to load the next element from the file
                            merge_data[i] =
                                match bincode::deserialize_from(merge_files.get_mut(i).unwrap()) {
                                    Ok(e) => Some(e),
                                    _ => None,
                                }
                        }
                    }
                }
            }
        }
    }

    // Generate a report
    let total_bytes = statistics.duplicate_chunk_bytes + statistics.unique_chunk_bytes;
    println!("{}s elapsed", started.elapsed().as_secs());
    println!("{} total bytes scanned", total_bytes);
    println!(
        "{} bytes {:0.4}% were unique",
        statistics.unique_chunk_bytes,
        ((statistics.unique_chunk_bytes * 100) as f64) / (total_bytes as f64)
    );
    println!("{} chunks", statistics.unique_chunks);
    println!(
        "{} bytes per chunk",
        statistics.unique_chunk_bytes / statistics.unique_chunks as u64
    );
    println!("{} collisions", statistics.collisions);
}

// Quickly stuffs all the entries in the btree into a file. The btreemap iterator is sorted, which we need.
fn write_memtree_file(
    mem_file_name: path::PathBuf,
    memtree: &mut collections::BTreeMap<[u8; 18], EntryData>,
) {
    let mem_file = fs::File::create(mem_file_name).unwrap();
    let mut buffer = io::BufWriter::new(&mem_file);
    let mut entry = Entry::default();
    for (key, value) in memtree.iter() {
        entry.key = *key;
        entry.size = value.size;
        entry.check = value.check;
        bincode::serialize_into(&mut buffer, &entry).unwrap();
    }
    memtree.clear();
}

// Call the specified callback function once for each file, recursing into sub-directories
fn visit_dirs(dir: &path::Path, callback: &mut dyn FnMut(&fs::DirEntry)) {
    let dir_result = fs::read_dir(dir);
    if !dir_result.is_ok() {
        return;
    }

    for entry in dir_result.unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.is_dir() {
            visit_dirs(&path, callback);
        } else {
            callback(&entry);
        }
    }
}

// Run either a variable-sized or fixed-size chunking algorithm on the specified file. Call the specified callback
// function once for each chunk found.
fn chunk_file(path: &path::Path, fixed_size: bool, callback: &mut dyn FnMut(&[u8])) {
    // Open the file if we can
    let file = fs::OpenOptions::new().read(true).open(path);
    if !file.is_ok() {
        return;
    }
    let file = file.unwrap();

    // Can't mmap zero-length files
    let metadata = file.metadata().unwrap();
    if 0 == metadata.len() {
        return;
    }

    // Chunk it
    let mmap = unsafe { memmap::Mmap::map(&file).unwrap() };
    if fixed_size {
        let remainder: &[u8] = &mmap;
        for chunk in remainder.chunks(4096) {
            callback(chunk);
        }
    } else {
        let chunker = rabin::chunker::Chunker::new(&mmap, MIN_CHUNK_SIZE, MAX_CHUNK_SIZE);
        for chunk in chunker {
            callback(chunk);
        }
    }
}

// SHA2 and SHA3 are completely different algorithms. It is extremely unlikely that and particular piece of data will
// have even just these four bytes of SHA2 match another piece of data with the same SHA3 hash.
fn sha2_check(chunk: &[u8]) -> u32 {
    let hash = rabin::hash_chunk_sha256(chunk);

    (hash[0] as u32) << 24 | (hash[1] as u32) << 16 | (hash[2] as u32) << 8 | (hash[3] as u32)
}

fn parse_memory_usage(mem_str: &str) -> u64 {
    let re = regex::Regex::new(r"(\d+)([bBkKmMgG]?)").unwrap();
    let caps = re.captures(mem_str).unwrap();

    let bytes = caps
        .get(1)
        .map_or("0", |m| m.as_str())
        .parse::<u64>()
        .unwrap();
    match caps.get(2).map_or("", |m| m.as_str()) {
        "G" => bytes * 1024 * 1024 * 1024,
        "g" => bytes * 1024 * 1024 * 1024,
        "M" => bytes * 1024 * 1024,
        "m" => bytes * 1024 * 1024,
        "K" => bytes * 1024,
        "k" => bytes * 1024,
        _ => bytes,
    }
}

struct Statistics {
    unique_chunks: u32,
    duplicates: u32,
    unique_chunk_bytes: u64,
    duplicate_chunk_bytes: u64,
    collisions: u32,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Serialize, Deserialize)]
struct Entry {
    key: [u8; KEY_LEN],
    size: u16,
    check: u32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct EntryData {
    check: u32,
    size: u16,
}

#[cfg(test)]
mod tests {

    #[test]
    fn test_parse_memory_usage() {
        assert_eq!(crate::parse_memory_usage("100"), 100u64);
        assert_eq!(crate::parse_memory_usage("100b"), 100u64);
        assert_eq!(crate::parse_memory_usage("100B"), 100u64);
        assert_eq!(crate::parse_memory_usage("100k"), 100u64 * 1024);
        assert_eq!(crate::parse_memory_usage("100K"), 100u64 * 1024);
        assert_eq!(crate::parse_memory_usage("100m"), 100u64 * 1024 * 1024);
        assert_eq!(crate::parse_memory_usage("100M"), 100u64 * 1024 * 1024);
        assert_eq!(
            crate::parse_memory_usage("100g"),
            100u64 * 1024 * 1024 * 1024
        );
        assert_eq!(
            crate::parse_memory_usage("100G"),
            100u64 * 1024 * 1024 * 1024
        );
    }
}
