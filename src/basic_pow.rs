//! Basic PoW 10 times

use sha2::{Sha256, Digest};
use std::time::{SystemTime, UNIX_EPOCH};
use serde::{Serialize, Deserialize};
use std::env;
use num_bigint::BigInt;

struct BitcoinBlockHeader {
  version: u32,
  prev_block_hash: [u8; 32],
  timestamp: u32,
  difficulty_target: u32,
  nonce: u32,
}

impl BitcoinBlockHeader {
  // Concatenate block header fields into bytes
  fn serialize(&self) -> Vec<u8> {
      let mut serialized_header = Vec::new();
      serialized_header.extend(&self.version.to_le_bytes());
      serialized_header.extend(&self.prev_block_hash);
      serialized_header.extend(&self.timestamp.to_le_bytes());
      serialized_header.extend(&self.difficulty_target.to_le_bytes());
      serialized_header.extend(&self.nonce.to_le_bytes());
      serialized_header
  }

  // Calculate the hash of the block header
  fn calculate_hash(&self) -> [u8; 32] {
      let serialized_header = self.serialize();
      let hash = Sha256::digest(&Sha256::digest(&serialized_header));
      hash.into()
  }

  // Check if the hash meets the target difficulty
  fn meets_target(&self, target: &BigInt) -> bool {
      let hash = BigInt::from_bytes_be(num_bigint::Sign::Plus, &self.calculate_hash());
      hash <= *target
  }

  // Function to convert difficulty target (bits) to target value
  fn get_target_bitwise(nbits: u32) -> BigInt {
    let significand = nbits & 0x007f_ffff;
    let exponent = (nbits >> 24) as u32;
    let target_bytes: BigInt = BigInt::from(significand) << (8 * (exponent - 3));

    target_bytes
  }
  
  fn get_target(nbits: u32) -> BigInt 
  {
    let significand: u32 = (nbits << 8) >> 8;  // 1 ~ 3 bytes of nBits
    let exponent: u32 = (nbits >> 24) as u32; // first byte of nBits
    let target: BigInt = significand * BigInt::from(256).pow(exponent - 3);
    target
  }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BlockMetaData {
  version: &'static str,
  previous_hash: String,
  index: u128,
  /// in milliseconds
  block_generation_interval: u128,
  /// how many blocks
  difficulty_adjustment_interval: u128,
  difficulty: usize,
  timestamp: u128,
}

// Struct for the Block Header
#[derive(Debug, Clone)]
struct BlockHeader {
  metadata: BlockMetaData,
  nonce: u128,
  hash: String,
}

// Struct for the Block
#[derive(Debug)]
struct Block {
  header: BlockHeader,
  /// instead of transaction list
  data: String,
}

impl Block {
  /// Create a new block
  fn new(
      version: &'static str,
      previous_hash: String,
      index: u128,
      difficulty: usize,
      timestamp: u128,
      nonce: u128,
      hash: String,
      data: String,
    ) -> Block {
      
      let gen_interval = match std::env::var("block_generation_interval") {
        Ok(interval_) => interval_.parse::<u128>(),
        _ => panic!("no gen interval"),
      };

      let diff_interval = match std::env::var("difficulty_adjustment_interval") {
        Ok(interval_) => interval_.parse::<u128>(),
        _ => panic!("no diff interval"),
      };
      
      let metadata = match (gen_interval, diff_interval) {
        (Ok(gen_), Ok(diff_)) => BlockMetaData {
          version, 
          previous_hash, 
          index, 
          difficulty, 
          block_generation_interval: gen_, 
          difficulty_adjustment_interval: diff_, 
          timestamp 
        },
        _ => panic!("no metadata"),
      };
      
      let header = BlockHeader {
        metadata,
        hash, 
        nonce,
      };

    let block = Block { header, data };

    block
  }
  
  /// Start the mining process
  fn find_block(metadata: &BlockMetaData, data: String) -> Block {
    let mut nonce = 0; 
    loop {
      let hash = Self::calculate_hash(metadata, nonce, &data);
        
      if Self::has_matches_difficulty(&hash, metadata.difficulty) {
        println!("Find new hash!: {}",hash);

        return Block::new(
          metadata.version,
          metadata.previous_hash.clone(),
          metadata.index,
          metadata.difficulty,
          metadata.timestamp,
          nonce,
          hash,
          data
        )
      }
      nonce += 1;
    }
  }

  /// Calculate the hash of the block
  fn calculate_hash(metadata: &BlockMetaData, nonce: u128, data: &String) -> String {
    let mut hasher = Sha256::new();
    let blockdata = serde_json::json!(metadata);
    hasher.update(blockdata.to_string().as_bytes());
    hasher.update(nonce.to_be_bytes());
    hasher.update(data.as_bytes());
    let hash = hasher.finalize();
    let hash_str = format!("{:02x}", hash);
    
    hash_str
  }

  fn has_matches_difficulty(hash: &String, difficulty: usize) -> bool {
    let hash_in_binary = hash.as_bytes();
    let required_prefix = &b"0".repeat(difficulty);

    hash_in_binary.starts_with(required_prefix)
  }
}

/// Struct for the Blockchain
#[derive(Debug)]
struct Blockchain {
  chain: Vec<Block>,
}

impl Blockchain {
  /// Create a new blockchain with a genesis block
  fn start() -> Self {
    
    let gen_interval = match std::env::var("block_generation_interval") {
      Ok(interval_) => interval_.parse::<u128>(),
      _ => panic!("no gen interval"),
    };

    let diff_interval = match std::env::var("difficulty_adjustment_interval") {
      Ok(interval_) => interval_.parse::<u128>(),
      _ => panic!("no diff interval"),
    };
    
    let metadata = match (gen_interval, diff_interval) {
      (Ok(gen_), Ok(diff_)) => BlockMetaData {
        version: "0.0.1",
        previous_hash: String::new(),
        index: 0, 
        difficulty: 3, 
        block_generation_interval: gen_, 
        difficulty_adjustment_interval: diff_, 
        timestamp: 1710077682714u128 // fixed
      },
      _ => panic!("no metadata"),
    };
    
    let (nonce, data) = (0u128, "Genesis Block".to_owned());

    let header = BlockHeader {
      metadata: metadata.clone(),
      hash: Block::calculate_hash(&metadata, nonce, &data),
      nonce,
    };

    let genesis_block = Block { header, data };
    
    Blockchain {
      chain: vec![genesis_block],
    }
  }

  /// Add a new block to the blockchain
  fn add_block(&mut self, data: String) {
    let latest_block = self.chain.last();

    let previous_hash = match latest_block {
      Some(block_) => block_.header.hash.clone(),
      _ => panic!("No prev hash"),
    };
    let previous_index = match latest_block {
      Some(block_) => block_.header.metadata.index,
      _ => panic!("No prev index"),
    };

    let gen_interval = match std::env::var("block_generation_interval") {
      Ok(interval_) => interval_.parse::<u128>(),
      _ => panic!("no gen interval"),
    };

    let diff_interval = match std::env::var("difficulty_adjustment_interval") {
      Ok(interval_) => interval_.parse::<u128>(),
      _ => panic!("no diff interval"),
    };

    let difficulty = self.get_difficulty();
    println!("get diff: {}", difficulty);

    const VERSION: &str = env!("CARGO_PKG_VERSION");

    let metadata = match (gen_interval, diff_interval) {
      (Ok(gen_), Ok(diff_)) => BlockMetaData {
        version: VERSION,
        previous_hash,
        index: previous_index + 1, 
        difficulty, 
        block_generation_interval: gen_, 
        difficulty_adjustment_interval: diff_,
        timestamp: Blockchain::current_timestamp()
      },
      _ => panic!("no metadata"),
    };
  
    let new_block = Block::find_block(&metadata, data);
    self.chain.push(new_block);
  }

  fn current_timestamp() -> u128 {
    let now = SystemTime::now().duration_since(UNIX_EPOCH);
    match now {
      Ok(now_) => now_.as_millis(),
      _ => panic!("systemtime does not work")
    }
  }

  fn get_difficulty(&self) -> usize {
    let latest_block = self.chain.last();
    
    let check_interval = match latest_block {
      Some(block_) 
      => {
        block_.header.metadata.index.rem_euclid(block_.header.metadata.difficulty_adjustment_interval) == 0
        && block_.header.metadata.index != 0 },
      _ => panic!("cannot get difficulty"),
    };
    
    if check_interval {
      match latest_block {
        Some(block_) => { 
          Blockchain::get_adjusted_difficulty(&self, block_)
        },
        _ => panic!(""),
      }
    } else {
      match latest_block {
        Some(block_) => block_.header.metadata.difficulty,
        _ => panic!(""),
      }
    }
  }

  fn get_adjusted_difficulty(&self, block: &Block) -> usize {
    let prev_adjustment_block = &self.chain[self.chain.len() - block.header.metadata.difficulty_adjustment_interval as usize];
    let time_expected = block.header.metadata.block_generation_interval * block.header.metadata.difficulty_adjustment_interval;
    let time_taken = block.header.metadata.timestamp - prev_adjustment_block.header.metadata.timestamp;
    
    println!("cal diff: {}, {}  ({})", time_taken, time_expected, prev_adjustment_block.header.metadata.difficulty);
    
    if time_taken < (time_expected / 2u128) {
      prev_adjustment_block.header.metadata.difficulty + 1
    } else if time_taken > (time_expected * 2u128) {
      prev_adjustment_block.header.metadata.difficulty - 1
    } else {
      prev_adjustment_block.header.metadata.difficulty
    }
  }
}

pub fn main() {
  let mut bc = Blockchain::start();
  for i in 1..=10 {
    bc.add_block(format!("Block {} Data", i));
  }
  println!("{:#?}", bc);
  println!("------------------------------");
  // Example values for the block header
  let block_header = BitcoinBlockHeader {
    version: 1,
    prev_block_hash: [0; 32],
    timestamp: 1617439785,
    difficulty_target: 0x1b0404cb, // Example difficulty target (bits)
    nonce: 0,
  };

  // Get target from difficulty target
  let target = BitcoinBlockHeader::get_target(block_header.difficulty_target);
  let target_bitwise = BitcoinBlockHeader::get_target_bitwise(block_header.difficulty_target);

  // Calculate hash and check if it meets the target
  let hash = block_header.calculate_hash();
  println!("Block hash: {:?}", hash);
  println!("Target comparison: {} vs {} =? {}", target, target_bitwise, target==target_bitwise);
  println!("Meets target: {}", block_header.meets_target(&target));
}