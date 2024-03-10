//! Two node PoW

use core::{panic, time};
use std::env;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use async_std::net::{TcpListener, TcpStream};
use async_std::stream::StreamExt;
use async_std::task;
use futures::io::BufReader;
use futures::{AsyncReadExt, AsyncWriteExt}; 
use serde::{Serialize, Deserialize};
use serde_json::from_str;
use sha2::{Sha256, Digest};

/*
This code is referencing following links:

참고 자료:
https://lhartikk.github.io/jekyll/update/2017/07/13/chapter2.html
https://medium.com/@lfoster49203/lets-create-a-cryptocurrency-in-rust-how-i-created-the-lyroncoin-cryptocurrency-a32a978030d4
https://blog.logrocket.com/how-to-build-a-blockchain-in-rust/

참고 코드:
https://github.com/chrishayuk/chrishayuk-monorepo/tree/main/apps
https://github.com/serde-rs/json/issues/657
*/


#[derive(Debug, Clone, Serialize, Deserialize)]
struct BlockMetaData {
  version: String,
  previous_hash: String,
  index: u128,
  /// in milliseconds
  block_generation_interval: u128, 
  /// in blocks 
  difficulty_adjustment_interval: u128,
  difficulty: usize,
  timestamp: u128,
}

/// 블록의 헤더
#[derive(Debug, Clone, Serialize, Deserialize)]
struct BlockHeader{
  metadata: BlockMetaData,
  nonce: u128,
  hash: String,
}

/// Struct for the Block
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Block {
  header: BlockHeader,
  /// instead of transaction list
  data: String, 
}

impl Block{
  /// Create a new block
  fn new(
      version: String,
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
          version: version.to_owned(), 
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

  async fn _find_block(metadata: BlockMetaData, data: String, stream:&mut TcpStream, mx:&Arc<Mutex<Option<Block>>>, choice:String) -> Block {
    match choice.as_str() {
      "plus 1" => {
        let mut nonce = 0;
        loop {
          let mut received_block = mx.lock().expect("no mutex value");
          let received_block_ = received_block.clone();
          *received_block = None;
          drop(received_block);
          let hash: String = Self::_calculate_hash(metadata.clone(), nonce, &data);
 
          if let Some(block_) = &received_block_ { 
            println!("received block (1): {:?}", received_block_);

            let received_hash: String = Self::_calculate_hash(
              block_.header.metadata.clone(),
              block_.header.nonce, 
              &block_.data
            );
            // Verify the validity of the received block hash
            if block_.header.hash == received_hash 
            && block_.header.metadata.previous_hash == metadata.previous_hash 
            && block_.header.metadata.timestamp <= metadata.timestamp { 
              
              if block_.header.metadata.timestamp == metadata.timestamp {
                for i in 0..block_.header.hash.as_bytes().len() {
                  if block_.header.hash.as_bytes()[i].to_ascii_lowercase() != hash.as_bytes()[i].to_ascii_lowercase() {
                    println!("hash bytes: {}, {}", block_.header.hash.as_bytes()[i].to_ascii_lowercase() , hash.as_bytes()[i].to_ascii_lowercase());
                    return match block_.header.hash.as_bytes()[i].to_ascii_lowercase() < hash.as_bytes()[i].to_ascii_lowercase() {
                      true => block_.to_owned(),
                      false => Block::new(
                        metadata.clone().version,
                        metadata.previous_hash.clone(),
                        metadata.index,
                        metadata.difficulty,
                        metadata.timestamp,
                        nonce,
                        hash.clone(),
                        data.clone()
                      ),
                    }
                  }
                }
              }
              // println!("Block verified: {:#?}", block_);

              return Block::new(
                block_.header.metadata.version.to_owned(),
                block_.header.metadata.previous_hash.clone(),
                block_.header.metadata.index,
                block_.header.metadata.difficulty,
                block_.header.metadata.timestamp,
                block_.header.nonce,
                block_.header.hash.clone(),
                block_.data.clone()
              );
            }
          }
          

          if Self::_has_matches_difficulty(&hash, metadata.difficulty) 
          {
            println!("Find new hash!: {}\n(nonce_{}) at time {}", hash, nonce, metadata.timestamp);

            let new_block = Block::new(
              metadata.clone().version,
              metadata.previous_hash.clone(),
              metadata.index,
              metadata.difficulty,
              metadata.timestamp,
              nonce,
              hash.clone(),
              data.clone()
            );

            let mut bytes: Vec<u8> = Vec::new();
            serde_json::to_writer(&mut bytes, &Some(&new_block)).unwrap();
            let _ = stream.write_all(&bytes).await;
            // println!("sent from server:\nhash ({})\nnonce ({})", new_block.header.hash, new_block.header.nonce);
            stream.flush().await.expect("flush?");
            
            println!("\n##### waiting before adding block to chain...");
            std::thread::sleep(time::Duration::from_millis(5000));
            let received_block = mx.lock().expect("no mutex value");
            let received_block_ = received_block.clone();
            // *received_block = None;
            // drop(received_block);

            println!("received block (2): {:?}", received_block_);

            if let Some(block_) = &received_block_ {
              if block_.header.metadata.previous_hash == metadata.previous_hash
              && block_.header.metadata.timestamp <= metadata.timestamp { 
                if block_.header.metadata.timestamp == metadata.timestamp {
                  for i in 0..block_.header.hash.as_bytes().len() {
                    if block_.header.hash.as_bytes()[i].to_ascii_lowercase() != hash.as_bytes()[i].to_ascii_lowercase() {
                      println!("hash bytes: {}, {}", block_.header.hash.as_bytes()[i].to_ascii_lowercase() , hash.as_bytes()[i].to_ascii_lowercase());
                      return match block_.header.hash.as_bytes()[i].to_ascii_lowercase() < hash.as_bytes()[i].to_ascii_lowercase() {
                        true => block_.to_owned(),
                        false => new_block,
                      }
                    }
                  }
                }
                // continue; 
              }
            }
            return new_block;
          }
          nonce += 1;
        }
      },
      "minus 1" => {
        let mut nonce = u128::MAX;
        loop {
          let mut received_block = mx.lock().expect("no mutex value");
          let received_block_ = received_block.clone();
          *received_block = None; 
          drop(received_block);
          let hash: String = Self::_calculate_hash(metadata.clone(), nonce, &data);

          if let Some(block_) = &received_block_ { 
            println!("received block (1): {:?}", received_block_);

            let received_hash: String = Self::_calculate_hash(
              block_.header.metadata.clone(), 
              block_.header.nonce, 
              &block_.data
            );
            
            // Verify the validity of the received block hash
            if block_.header.hash == received_hash 
            && block_.header.metadata.previous_hash == metadata.previous_hash 
            && block_.header.metadata.timestamp <= metadata.timestamp {
              // println!("Block verified: {:#?}", block_);
              if block_.header.metadata.timestamp == metadata.timestamp {
                for i in 0..block_.header.hash.as_bytes().len() {
                  if block_.header.hash.as_bytes()[i].to_ascii_lowercase() != hash.as_bytes()[i].to_ascii_lowercase() {
                    println!("hash bytes: {}, {}", block_.header.hash.as_bytes()[i].to_ascii_lowercase() , hash.as_bytes()[i].to_ascii_lowercase());
                    return match block_.header.hash.as_bytes()[i].to_ascii_lowercase() < hash.as_bytes()[i].to_ascii_lowercase() {
                      true => block_.to_owned(),
                      false => Block::new(
                        metadata.clone().version,
                        metadata.previous_hash.clone(),
                        metadata.index,
                        metadata.difficulty,
                        metadata.timestamp,
                        nonce,
                        hash.clone(),
                        data.clone()
                      ),
                    }
                  }
                }
              }

              return Block::new(
                block_.header.metadata.version.to_owned(),
                block_.header.metadata.previous_hash.clone(),
                block_.header.metadata.index,
                block_.header.metadata.difficulty,
                block_.header.metadata.timestamp,
                block_.header.nonce,
                block_.header.hash.clone(),
                block_.data.clone()
              );
            }
          }

          if Self::_has_matches_difficulty(&hash, metadata.difficulty) 
          {
            println!("Find new hash!: {}\n(nonce_{}) at time {}", hash, nonce, metadata.timestamp);

            let new_block = Block::new(
              metadata.clone().version,
              metadata.previous_hash.clone(),
              metadata.index,
              metadata.difficulty,
              metadata.timestamp,
              nonce,
              hash.clone(),
              data.clone()
            );

            let mut bytes: Vec<u8> = Vec::new();
            serde_json::to_writer(&mut bytes, &Some(&new_block)).unwrap();
            let _ = stream.write_all(&bytes).await;
            // println!("sent from server:\nhash ({})\nnonce ({})", new_block.header.hash, new_block.header.nonce);
            stream.flush().await.expect("flush?");

            println!("\n##### waiting before adding block to chain...");
            std::thread::sleep(time::Duration::from_millis(5000));
            let received_block = mx.lock().expect("no mutex value");
            let received_block_ = received_block.clone();
            // *received_block = None; 
            // drop(received_block);

            println!("received block (2): {:?}", received_block_);

            if let Some(block_) = &received_block_ {
              if block_.header.metadata.previous_hash == metadata.previous_hash 
              && block_.header.metadata.timestamp <= metadata.timestamp { 
                if block_.header.metadata.timestamp == metadata.timestamp {
                  for i in 0..block_.header.hash.as_bytes().len() {
                    if block_.header.hash.as_bytes()[i].to_ascii_lowercase() != hash.as_bytes()[i].to_ascii_lowercase() {
                      println!("hash bytes: {}, {}", block_.header.hash.as_bytes()[i].to_ascii_lowercase() , hash.as_bytes()[i].to_ascii_lowercase());
                      return match block_.header.hash.as_bytes()[i].to_ascii_lowercase() < hash.as_bytes()[i].to_ascii_lowercase() {
                        true => block_.to_owned(),
                        false => new_block,
                      }
                    }
                  }
                }
                // continue; 
              }
            }
            return new_block;
          }
          nonce -= 1;
        }
      },
      _ => panic!("no choice")
    };
  }


  /// Calculate the hash of the block
  fn _calculate_hash(metadata: BlockMetaData, nonce: u128, data: &String) -> String {
    let mut hasher = Sha256::new();
    let blockdata = serde_json::to_string(&metadata).expect("json string failed");
    hasher.update(blockdata.as_bytes());
    hasher.update(nonce.to_be_bytes());
    hasher.update(data.as_bytes());
    let hash = hasher.finalize();
    let hash_str = format!("{:02x}", hash);
    
    hash_str
  }

  fn _has_matches_difficulty(hash: &String, difficulty: usize) -> bool {
    let hash_in_binary = hash.as_bytes();
    let required_prefix = &b"0".repeat(difficulty);

    hash_in_binary.starts_with(required_prefix)
  }
}

// Struct for the Blockchain
#[derive(Debug, Clone)]
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

    let begining_difficulty = match std::env::var("difficulty") {
      Ok(diffic_) => diffic_.parse::<usize>(),
      _ => panic!("no difficulty"),
    };

    let metadata = match (gen_interval, diff_interval, begining_difficulty) {
      (Ok(gen_), Ok(diff_), Ok(diffic_)) => BlockMetaData {
        version: "0.0.1".to_owned(),
        previous_hash: String::new(),
        index: 0, 
        difficulty: diffic_, 
        block_generation_interval: gen_, 
        difficulty_adjustment_interval: diff_, 
        timestamp: 1710077682714u128 // Fixed
      },
      _ => panic!("no metadata"),
    };
    
    let (nonce, data) = (0u128, "Genesis Block".to_owned());

    let header = BlockHeader {
      metadata: metadata.clone(),
      hash: Block::_calculate_hash(metadata, nonce, &data),
      nonce,
    };

    let genesis_block = Block { header, data };
    
    Blockchain {
      chain: vec![genesis_block],
    }
  }

  /// Add a new block to the blockchain
  async fn add_block(&mut self, data: String, stream:&mut TcpStream, mx:Arc<Mutex<Option<Block>>>, choice:String) {

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

    let difficulty = self._get_difficulty();

    const VERSION: &str = env!("CARGO_PKG_VERSION");

    let metadata = match (gen_interval, diff_interval) {
      (Ok(gen_), Ok(diff_)) => BlockMetaData {
        version: VERSION.to_owned(),
        previous_hash,
        index: previous_index + 1, 
        difficulty, 
        block_generation_interval: gen_, 
        difficulty_adjustment_interval: diff_,
        timestamp: Blockchain::current_timestamp()
      },
      _ => panic!("no metadata"),
    };
    
    println!("Difficulty (leading zeros): {}, start to find block!", difficulty);

    let new_block = Block::_find_block(metadata, data, stream, &mx, choice).await;
    // println!("new block: {:#?}", new_block);

    self.chain.push(new_block);
    println!("\n------------------------");
    for b in &self.chain {
      println!("blockchain nonces: {}",b.header.nonce);
    }
  }

  /// Get the current timestamp 
  fn current_timestamp() -> u128 {
    let now = SystemTime::now().duration_since(UNIX_EPOCH);
    match now {
      Ok(now_) => now_.as_millis(),
      _ => panic!("systemtime does not work")
    }
  }

  fn _get_difficulty(&self) -> usize {
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
          Blockchain::_get_adjusted_difficulty(&self, block_)
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

  fn _get_adjusted_difficulty(&self, block: &Block) -> usize {
    let prev_adjustment_block = &self.chain[self.chain.len() - block.header.metadata.difficulty_adjustment_interval as usize];
    let time_expected = block.header.metadata.block_generation_interval * block.header.metadata.difficulty_adjustment_interval;
    let time_taken = block.header.metadata.timestamp - prev_adjustment_block.header.metadata.timestamp;
    
    println!("Calculate difficulty: Time taken({}), Time expected({}) | Previous difficulty: ({})", time_taken, time_expected, prev_adjustment_block.header.metadata.difficulty);
    
    if time_taken < (time_expected / 2u128) {
      prev_adjustment_block.header.metadata.difficulty + 1
    } else if time_taken > (time_expected * 2u128) {
      prev_adjustment_block.header.metadata.difficulty - 1
    } else {
      prev_adjustment_block.header.metadata.difficulty
    }
  }
}

async fn start_blockchain(port:String, choice:String, block_signal:Arc<Mutex<Option<Block>>>) {
  std::thread::sleep(time::Duration::from_millis(5000));
  
  let mut bc = Blockchain::start();
  let mut i = 0;
  loop {
    let mut stream = TcpStream::connect(format!("127.0.0.1:{}", port)).await.expect("block on failed");
    let mut buffer = [0u8; 1024];
    let (reader, _) = stream.clone().split();
    let mut reader = BufReader::new(reader);
    let len = reader.read(&mut buffer).await.expect("cannot read message");
    println!("{}", String::from_utf8_lossy(&buffer[..len]));

    let choice = choice.clone();
    let block_signal_ = block_signal.clone();
    
    bc.add_block(format!("Block of `{}` insert {:?} Data", choice, i), &mut stream, block_signal_, choice).await;
    println!("Length of chain: {}", bc.chain.len());
    println!("------------------------\n");

    i += 1;
  }
}

/// Start the socket server
async fn start_tcp_socket(port:String, block_signal:Arc<Mutex<Option<Block>>>) {
  let listener = TcpListener::bind(format!("127.0.0.1:{}", port)).await.expect("Failed to start server");
  println!("Listening started on {}, ready to accept connections", port);

  let mut incoming = listener.incoming();
  loop {
    match incoming.next().await.expect("incoming failed") {
      Ok(mut stream) => {
        let block_signal_ = block_signal.clone();
        task::spawn(async move {
          handle_client(&mut stream, block_signal_).await;
        });
      },
      _ => {
        println!("not connected");
      }
    }
  }
}

/// Function to handle each client connection
async fn handle_client(stream: &mut TcpStream, block_signal:Arc<Mutex<Option<Block>>>) {
  stream.write_all(b"Hello Blockchain\r\n").await.expect("Failed to write data to stream");
  stream.flush().await.expect("not flushed");

  let peer = stream.peer_addr().expect("no peer");
  println!("##### Client connected: ({})", peer);

  let mut buffer: Vec<u8> = Vec::new();
  
  loop {
    let len = stream.read_to_end(&mut buffer).await.expect("no len");
    let message = String::from_utf8_lossy(&buffer[..len]);
    
    if message.len() != 0 {
      let received_block = from_str::<Option<Block>>(&message)
                                          .expect("no received block");
      // println!("received block from client: {:#?}",received_block);
      
      let mut block_signal_lock = block_signal.lock().expect("no mutex value");
      *block_signal_lock = received_block;
      drop(block_signal_lock);
      break;
    }
  }
  
}

#[async_std::main]
pub async fn main() {
  let args: Vec<String> = env::args().collect();
  let bind_port = &args[1];
  let conn_port = &args[2];
  let choice = &args[3];
  
  let m1 = Mutex::new(None);
  let block_signal: Arc<Mutex<Option<Block>>> = Arc::new(m1);

  futures::join!(start_tcp_socket(bind_port.clone(), block_signal.clone()),start_blockchain(conn_port.clone(), choice.clone(), block_signal.clone()));
}