use in_place_fastx;
use std::time::Instant;

use rayon::iter::ParallelBridge;
use rayon::iter::ParallelIterator;

use needletail::FastxReader;

const K: usize = 13;
const BLOCK_SIZE: u64 = 2u64.pow(21);

in_place_fastx::fasta_sharedstate!(
    Parser,
    u64,
    |record: in_place_fastx::block::Record, _dummy: &u64| {
    }
);

// A small examaple with dummy worker task that does nothing

fn main() -> in_place_fastx::error::Result<()> {
    let parser = Parser::new();

    let mut args = std::env::args();
    let _ = args.next();
    let mut _test:u64 = 0;
    let input = args.next().unwrap();


    let start = Instant::now();
    parser.with_blocksize(BLOCK_SIZE, input.clone(), &_test)?;
    let duration = start.elapsed();
    println!("in_place_fastx parsed in {:?}.", duration);

    println!("now needletail:");

    let start = Instant::now();
    let mut reader = needletail::parser::FastaReader::with_capacity(
        std::fs::File::open(input).unwrap(),
        BLOCK_SIZE as usize,
    );

    while let Some(Ok(_record)) = reader.next() {
    }
    let duration = start.elapsed();
    println!("needletail parsed in {:?}.", duration);

    Ok(())
}
