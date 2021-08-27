//! Struct that extract part of file (called block) and read it as fastq file.

/* crate use */
use bstr::ByteSlice;

/* project use */
use crate::block;
use crate::block::Producer as AbcProducer;
use crate::error;

/// Struct that produce a [Block](super::Block) of file, this block contains complete record.
pub struct Producer {
    offset: u64,
    blocksize: u64,
    file: std::fs::File,
    file_length: u64,
}

impl Producer {}

impl block::Producer for Producer {
    /// Build a [Block](super::Block) producer, with a specific [Block](super::Block) size warning this block size must be larger than two records.
    fn with_blocksize<P>(blocksize: u64, path: P) -> Result<Self, error::Error>
    where
        P: AsRef<std::path::Path>,
    {
        Ok(Producer {
            offset: 0,
            blocksize: Self::fix_blocksize::<P>(&path, blocksize)?,
            file_length: Self::filesize::<P>(&path)?,
            file: std::fs::File::open(path).map_err(|source| error::Error::OpenFile { source })?,
        })
    }

    /// Search the begin of the partial record at the end of [Block](super::Block)
    fn correct_block_size(block: &[u8]) -> error::Result<u64> {
        let mut end = block.len();

        for _ in 0..4 {
            end = block[..end]
                .rfind_byte(b'\n')
                .ok_or(error::Error::NoNewLineInBlock)?;

            if end + 1 < block.len() && block[end + 1] == b'@' {
                let prev = block[..end]
                    .rfind_byte(b'\n')
                    .ok_or(error::Error::NoNewLineInBlock)?;
                if block[prev + 1] == b'+' {
                    let prevprev = block[..prev]
                        .rfind_byte(b'\n')
                        .ok_or(error::Error::NoNewLineInBlock)?;
                    if block[prevprev + 1] == b'+' {
                        return Ok((end + 1) as u64);
                    } else {
                        return Err(error::Error::NotAFastqFile);
                    }
                } else {
                    return Ok((end + 1) as u64);
                }
            }
        }

        Err(error::Error::NotAFastqFile)
    }

    /// Get current value of offset
    fn offset(&self) -> u64 {
        self.offset
    }

    /// Get file length
    fn file_length(&self) -> u64 {
        self.file_length
    }

    /// Get file
    fn file(&self) -> &std::fs::File {
        &self.file
    }

    /// Get blocksize
    fn blocksize(&self) -> u64 {
        self.blocksize
    }

    /// Set value of offset
    fn set_offset(&mut self, value: u64) {
        self.offset = value;
    }
}

impl Iterator for Producer {
    type Item = error::Result<block::Block>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.next_block() {
            Ok(Some(block)) => Some(Ok(block)),
            Ok(None) => None,
            Err(e) => Some(Err(e)),
        }
    }
}

/// Struct that read [Block](Block) and produce [Record](Record)
pub struct Reader {
    offset: usize,
    block: block::Block,
}

impl block::Reader for Reader {
    /// Create a new [Block](Block) reader from [Block](Block) get in parameter
    fn new(block: block::Block) -> Self {
        Reader { offset: 0, block }
    }

    /// Produce [Record](Record) until block is empty
    fn next_record(&mut self) -> error::Result<Option<block::Record<'_>>> {
        if self.offset == self.block.len() {
            Ok(None)
        } else {
            let comment = &self.block.data()[self.get_line()?];
            self.offset += comment.len() + 1;

            let sequence = &self.block.data()[self.get_line()?];
            self.offset += sequence.len() + 1;

            let plus = &self.block.data()[self.get_line()?];
            self.offset += plus.len() + 1;

            let quality = &self.block.data()[self.get_line()?];
            self.offset += quality.len() + 1;

            Ok(Some(crate::block::Record {
                comment,
                sequence,
                plus,
                quality,
            }))
        }
    }

    fn data(&self) -> &[u8] {
        self.block.data()
    }

    fn offset(&self) -> usize {
        self.offset
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod producer {
        use super::*;

        use std::io::Write;

        #[test]
        fn new() {
            let mut tmp = Producer::new(crate::tests::generate_fastq(42, 100, 150)).unwrap();

            let block = tmp.next_block().unwrap().unwrap();

            assert_eq!(block.len(), 30980);

            assert!(tmp.next_block().unwrap().is_none());
        }

        #[test]
        fn with_blocksize() {
            let mut tmp =
                Producer::with_blocksize(463, crate::tests::generate_fastq(42, 1_000, 150))
                    .unwrap();

            let block = tmp.next_block().unwrap().unwrap();

            assert_eq!(block.len(), 308);

            assert_eq!(
                String::from_utf8(block.data().to_vec()).unwrap(),
                "@0
TTAGATTATAGTACGGTATAGTGGTTACTATGTAGCCTAAGTGGCGCCCGTTGTAGAGGAATCCACTTATATAACACAGGTATAATCCGGACGGCATGCGCAGGCATGCCTATATTCTATGACAGCAGGATTATGGAAGATGGTGCTCTA
+0
^U3<L0PV{cnrl:8N`!:=mF8M0!0Ez/d{4j$=9f5rLeAQ-H.ptT3w6aMy8Z6O-dZ}2`UX=YJ-Etg`s&B%~F!kR7S8]@lTI<2-\\';v0}hU.(T*0VHGx,>Gze)*5rFv}k@RllOE2K)\"DQJvO)bl?(dDhh
".to_string()
            );
        }

        #[test]
        fn with_blocksize_buffer_larger_file() {
            let mut tmp =
                Producer::with_blocksize(8092, crate::tests::generate_fastq(44, 2, 150)).unwrap();

            let block = tmp.next_block().unwrap().unwrap();

            assert_eq!(block.len(), 616);
        }

        #[test]
        fn get_all_block() {
            let mut tmp = Producer::new(crate::tests::generate_fastq(42, 1_000, 150)).unwrap();

            let mut block_length = Vec::new();
            while let Ok(Some(block)) = tmp.next_block() {
                block_length.push(block.len());
            }

            assert_eq!(block_length, vec![65300, 65520, 65520, 65520, 49920]);
        }

        #[test]
        fn check_block() {
            let mut tmp =
                Producer::with_blocksize(800, crate::tests::generate_fastq(42, 5, 150)).unwrap();

            assert_eq!(
                String::from_utf8(tmp.next_block().unwrap().unwrap().data().to_vec()),
                Ok("@0
TTAGATTATAGTACGGTATAGTGGTTACTATGTAGCCTAAGTGGCGCCCGTTGTAGAGGAATCCACTTATATAACACAGGTATAATCCGGACGGCATGCGCAGGCATGCCTATATTCTATGACAGCAGGATTATGGAAGATGGTGCTCTA
+0
^U3<L0PV{cnrl:8N`!:=mF8M0!0Ez/d{4j$=9f5rLeAQ-H.ptT3w6aMy8Z6O-dZ}2`UX=YJ-Etg`s&B%~F!kR7S8]@lTI<2-\\';v0}hU.(T*0VHGx,>Gze)*5rFv}k@RllOE2K)\"DQJvO)bl?(dDhh
@1
AGTTATCGTGTACCTCCTAGCTTTTAGTTGTGCTTTAACAGTGTAACATTGGGACGCTATTACTCGCCGGTGAGGCGGTCTTCCTTGACTATACCGATCGTGGAGTTCATGCGCGCGGATCCCTCAGCGTTCTCGGGAAGCGCGAACAGA
+1
iCW?:KL~15\\E|MNRKY)S$?~~Ub}d)dY2LX:e@b^'<<$$e56W0fdV,<Y>Yd(J<5p6xt)z+OxuPXv?/_yH8z^%Sks1*nxm$<7*YdkvNPf:>YW=$uxZ)}[v/DlZm&EW(s(cMelx\"iEV3Hp]cz3%_T@\\Ms
".to_string())
            );
            assert_eq!(
                String::from_utf8(tmp.next_block().unwrap().unwrap().data().to_vec()),
                Ok("@2
AATGTCCCTCAATCCGCGGCATGGCTAAGTACCACCGTGGATGTAAATTTTTCAGTCGTCTCTTCATACTGTTCCTGTACTGTCAGGGATGCTCCCTTTCACAGAGCTCGTATAATCAGTAAACGCCACGGTCCTTTCTCTGTTAACCGC
+2
Ouf)Y|l;S1!tk[U9n2(NK=#Tmg,t+CSsUMaPs7{+V'~On{hc1NR}aY^YbYlg[}Fcq1K_$v1HG\"tRBj`||g>\\)2pU_QrnWO{c@;lw8B0+urH~$#K>:iSa-I-C#gDJ(9UUFubOeRHsDX3Ko`?T--iL+j
@3
TTGGGCATGAGGTTCACCGAAGGTGGCAGATATGCGCCATAAATTGACCAGGTTGTATCCAGCATTGGAAGAACGCACCCGGGGGGAGCACAGATCCTAGCAGTACACGCTCTGGGTCCTCTACGTCTTCGGAGTCTCTAGCTTGCCTTA
+3
:~vGLKg+n!*iJ\\.*wfxK)5Qmh%<:f^$nql7OB$}M/d.F,5[=>ZW*#f=0>Ao(@~lEHbSG1%,b_Uy2!zL%2GMB0O.t[#UcQ[]ufFZJ!K<kLgDNQlx)s8+75E^[-\"!1l[i<S#G\"B]xZ5?as*@8Laq`{@r
".to_string())
            );
            assert_eq!(
                String::from_utf8(tmp.next_block().unwrap().unwrap().data().to_vec()),
                Ok("@4
TCTATAGCTTGTCATGCCTTTCGATTGAGGGCGTCACCAAGCGAATTACTCGCTGATCCGTTCCCCGCCAATTCTGAGACTCCATAATCCTATCTGTGTCCCTAGGTGCCGTGTTCCGGTCGTGAGTTCGGCCCTTGCCTAAAGTTAATG
+4
myS=C|jEWnl,aC\\7!jv9[!vh/PAK}_H&<.o]qf|y@4L:?ssLg3N!v7/N5RyPHn=5%Fyh(4-Z:<6wf]^#t~0:i(X\\l-7]9olH9WLV~`L~JQ7ye7B1RSi2N$PuHwjj\\pb}J\\R~pe?j+X>R#p@MyqBBe*
".to_string())
            );
            assert!(tmp.next_block().is_ok());
            assert!(tmp.next_block().unwrap().is_none());
        }

        #[test]
        fn comment_start_by_plus() {
            let mut file = tempfile::NamedTempFile::new().unwrap();

            file.write(
                b"@0
TTAGATTATAGTACGG
+0
^U3<L0PV{cnrl:8N
@1
AGTTATCGTGTACCTC
+1
+CW?:KL~15\\E|MN
@2
AATGTCCCTCAATCCG
+2
+uf)Y|l;S1!tk[U9
@3
TTGGGCATGAGGTTCA
+3
+~vGLKg+n!*iJ\\K
",
            )
            .unwrap();

            let mut producer = Producer::with_blocksize(82, file).unwrap();

            assert_eq!(
                String::from_utf8(producer.next_block().unwrap().unwrap().data().to_vec()),
                Ok("@0
TTAGATTATAGTACGG
+0
^U3<L0PV{cnrl:8N
@1
AGTTATCGTGTACCTC
+1
+CW?:KL~15\\E|MN
"
                .to_string())
            );

            assert_eq!(
                String::from_utf8(producer.next_block().unwrap().unwrap().data().to_vec()),
                Ok("@2
AATGTCCCTCAATCCG
+2
+uf)Y|l;S1!tk[U9
@3
TTGGGCATGAGGTTCA
+3
+~vGLKg+n!*iJ\\K
"
                .to_string())
            );
        }

        #[test]
        fn not_a_fastq() {
            let mut file = tempfile::NamedTempFile::new().unwrap();

            file.write(
                b"@0
TTAGATTATAGTACGG
ATTATAT
+1
AGTTATCGTGTACCTC
+1
+CW?:KL~15\\E|MN
GTCCCTCAATCCG
+2
",
            )
            .unwrap();

            let mut producer = Producer::with_blocksize(82, file.path()).unwrap();

            assert!(producer.next_block().is_err());

            {
                let mut rewrite = file.reopen().unwrap();
                rewrite
                    .write(
                        b"+FAILLED FILE
+3
+TTGGGCATGAGGTTCA
@3ueauie
+~vGLKg+n!*iJ\\K
@iuiea
",
                    )
                    .unwrap();
            }

            let mut producer = Producer::with_blocksize(82, file.path()).unwrap();

            assert!(producer.next_block().is_err());

            let mut producer = Producer::with_blocksize(82, file).unwrap();
            assert!(producer.next().is_some());
            assert!(producer.next().unwrap().is_err());
        }
    }

    mod reader {
        use super::*;

        use crate::block::Reader as AbcReader;

        #[test]
        fn iterate_over_seq() {
            let mut producer =
                Producer::with_blocksize(500, crate::tests::generate_fastq(42, 5, 150)).unwrap();

            let mut comments = Vec::new();
            let mut seqs = Vec::new();
            let mut pluss = Vec::new();
            let mut quals = Vec::new();

            while let Ok(Some(block)) = producer.next_block() {
                let mut reader = Reader::new(block);

                while let Ok(Some(record)) = reader.next_record() {
                    comments.push(String::from_utf8(record.comment.to_vec()).unwrap());
                    seqs.push(String::from_utf8(record.sequence.to_vec()).unwrap());
                    pluss.push(String::from_utf8(record.plus.to_vec()).unwrap());
                    quals.push(String::from_utf8(record.quality.to_vec()).unwrap());
                }
            }

            assert_eq!(
                comments,
                vec![
                    "@0".to_string(),
                    "@1".to_string(),
                    "@2".to_string(),
                    "@3".to_string(),
                    "@4".to_string()
                ]
            );
            assert_eq!(
                seqs,
                vec![
		    "TTAGATTATAGTACGGTATAGTGGTTACTATGTAGCCTAAGTGGCGCCCGTTGTAGAGGAATCCACTTATATAACACAGGTATAATCCGGACGGCATGCGCAGGCATGCCTATATTCTATGACAGCAGGATTATGGAAGATGGTGCTCTA".to_string(),
		    "AGTTATCGTGTACCTCCTAGCTTTTAGTTGTGCTTTAACAGTGTAACATTGGGACGCTATTACTCGCCGGTGAGGCGGTCTTCCTTGACTATACCGATCGTGGAGTTCATGCGCGCGGATCCCTCAGCGTTCTCGGGAAGCGCGAACAGA".to_string(),
		    "AATGTCCCTCAATCCGCGGCATGGCTAAGTACCACCGTGGATGTAAATTTTTCAGTCGTCTCTTCATACTGTTCCTGTACTGTCAGGGATGCTCCCTTTCACAGAGCTCGTATAATCAGTAAACGCCACGGTCCTTTCTCTGTTAACCGC".to_string(), "TTGGGCATGAGGTTCACCGAAGGTGGCAGATATGCGCCATAAATTGACCAGGTTGTATCCAGCATTGGAAGAACGCACCCGGGGGGAGCACAGATCCTAGCAGTACACGCTCTGGGTCCTCTACGTCTTCGGAGTCTCTAGCTTGCCTTA".to_string(), "TCTATAGCTTGTCATGCCTTTCGATTGAGGGCGTCACCAAGCGAATTACTCGCTGATCCGTTCCCCGCCAATTCTGAGACTCCATAATCCTATCTGTGTCCCTAGGTGCCGTGTTCCGGTCGTGAGTTCGGCCCTTGCCTAAAGTTAATG".to_string()     ]
            );
            assert_eq!(
                pluss,
                vec![
                    "+0".to_string(),
                    "+1".to_string(),
                    "+2".to_string(),
                    "+3".to_string(),
                    "+4".to_string()
                ]
            );
            assert_eq!(
                quals,
                vec![
		    "^U3<L0PV{cnrl:8N`!:=mF8M0!0Ez/d{4j$=9f5rLeAQ-H.ptT3w6aMy8Z6O-dZ}2`UX=YJ-Etg`s&B%~F!kR7S8]@lTI<2-\\';v0}hU.(T*0VHGx,>Gze)*5rFv}k@RllOE2K)\"DQJvO)bl?(dDhh".to_string(),
		    "iCW?:KL~15\\E|MNRKY)S$?~~Ub}d)dY2LX:e@b^'<<$$e56W0fdV,<Y>Yd(J<5p6xt)z+OxuPXv?/_yH8z^%Sks1*nxm$<7*YdkvNPf:>YW=$uxZ)}[v/DlZm&EW(s(cMelx\"iEV3Hp]cz3%_T@\\Ms".to_string(),
		    "Ouf)Y|l;S1!tk[U9n2(NK=#Tmg,t+CSsUMaPs7{+V'~On{hc1NR}aY^YbYlg[}Fcq1K_$v1HG\"tRBj`||g>\\)2pU_QrnWO{c@;lw8B0+urH~$#K>:iSa-I-C#gDJ(9UUFubOeRHsDX3Ko`?T--iL+j".to_string(),
		    ":~vGLKg+n!*iJ\\.*wfxK)5Qmh%<:f^$nql7OB$}M/d.F,5[=>ZW*#f=0>Ao(@~lEHbSG1%,b_Uy2!zL%2GMB0O.t[#UcQ[]ufFZJ!K<kLgDNQlx)s8+75E^[-\"!1l[i<S#G\"B]xZ5?as*@8Laq`{@r".to_string(),
		    "myS=C|jEWnl,aC\\7!jv9[!vh/PAK}_H&<.o]qf|y@4L:?ssLg3N!v7/N5RyPHn=5%Fyh(4-Z:<6wf]^#t~0:i(X\\l-7]9olH9WLV~`L~JQ7ye7B1RSi2N$PuHwjj\\pb}J\\R~pe?j+X>R#p@MyqBBe*".to_string(),
		]
            );
        }
    }
}