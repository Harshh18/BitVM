use std::collections::HashMap;
use std::str::FromStr;

use bitcoin::hashes::{ripemd160, Hash};
use bitcoin::key::{Keypair, Secp256k1};
use bitcoin::Address;

const DELIMITER: char = '=';
const HASH_LEN: usize = 20;

pub type HashDigest = [u8; HASH_LEN];
pub type HashPreimage = [u8; HASH_LEN];
pub type U160 = [u32; 5];

fn hash(bytes: &[u8]) -> HashDigest {
    ripemd160::Hash::hash(bytes).to_byte_array()
}

fn hash_id(identifier: &str, index: Option<u32>, value: u32) -> String {
    // TODO ensure there is no DELIMITER in identifier or index
    match index {
        None => format!("{identifier}{DELIMITER}{value}"),
        Some(index) => format!("{identifier}_{index}{DELIMITER}{value}"),
    }
}

fn to_commitment_id(identifier: &str, index: Option<u32>) -> String {
    match index {
        None => format!("{identifier}"),
        Some(index) => format!("{identifier}{index}"),
    }
}

fn parse_hash_id(hash_id: &str) -> (&str, &str) {
    let split_vec: Vec<&str> = hash_id.splitn(2, DELIMITER).collect();
    (split_vec[0], split_vec[1])
}

fn _preimage(secret: &[u8], hash_id: &str) -> HashDigest {
    hash(&[secret, hash_id.as_bytes()].concat())
}

fn _hash_lock(secret: &[u8], hash_id: &str) -> HashDigest {
    hash(&_preimage(secret, hash_id))
}

fn preimage(secret: &[u8], identifier: &str, index: Option<u32>, value: u32) -> HashDigest {
    println!("Hash_id: {:?}", hash_id(identifier, index, value));
    _preimage(secret, &hash_id(identifier, index, value))
}

fn hash_lock(secret: &[u8], identifier: &str, index: Option<u32>, value: u32) -> HashDigest {
    hash(&_preimage(secret, &hash_id(identifier, index, value)))
}

pub trait Actor {
    fn script_pub_key() -> Address {
        // TODO: Implement properly
        eprintln!("Hardcoded winner address!");
        Address::from_str("tb1p9evrt83ma6e2jjc9ajagl2h0kqtz5y05nutg2xt2tn9xjcm29t0slwpyc9")
            .unwrap()
            .require_network(bitcoin::Network::Testnet)
            .unwrap()
    }
    fn hashlock(&mut self, identifier: &str, index: Option<u32>, value: u32) -> Vec<u8>;
    fn preimage(&mut self, identifier: &str, index: Option<u32>, value: u32) -> Vec<u8>;
}

pub struct Player<'a> {
    // We can get the secret with keypair.secret_bytes()
    keypair: Keypair,
    hashes: HashMap<String, [u8; HASH_LEN]>,
    //model:
    opponent: &'a Opponent,
}

impl Actor for Player<'_> {
    fn hashlock(&mut self, identifier: &str, index: Option<u32>, value: u32) -> Vec<u8> {
        let hash = hash_lock(&self.keypair.secret_bytes(), identifier, index, value);
        self.hashes
            .insert(hash_id(identifier, index, value), hash.clone());
        hash.to_vec()
    }

    // TODO: Implement Model struct
    fn preimage(&mut self, identifier: &str, index: Option<u32>, value: u32) -> Vec<u8> {
        let commitment_id = to_commitment_id(identifier, index);
        // TODO set commitment_id in model
        //self.model...
        preimage(&self.keypair.secret_bytes(), identifier, index, value).to_vec()
    }
}

impl<'a> Player<'a> {
    pub fn new(secret: &str, opponent: &'a Opponent) -> Self {
        let secp = Secp256k1::new();
        Self {
            keypair: Keypair::from_seckey_str(&secp, secret).unwrap(),
            hashes: HashMap::new(),
            opponent,
        }
    }

    // TODO: Implement remaining functions from js version
}

pub struct Opponent {
    id_to_hash: HashMap<String, HashDigest>,
    hash_to_id: HashMap<HashDigest, String>,
    preimages: HashMap<String, HashDigest>,
    commitments: HashMap<String, String>,
}

impl Actor for Opponent {
    fn hashlock(&mut self, identifier: &str, index: Option<u32>, value: u32) -> Vec<u8> {
        let id = hash_id(identifier, index, value);
        self.id_to_hash
            .get(&id)
            .expect(&format!("Hash for {id} is not known"))
            .to_vec()
    }

    // TODO: Implement Model struct
    fn preimage(&mut self, identifier: &str, index: Option<u32>, value: u32) -> Vec<u8> {
        let id = hash_id(identifier, index, value);
        self.preimages
            .get(&id)
            .expect(&format!("Preimage of {id} is not known"))
            .to_vec()
    }
}

impl Opponent {
    pub fn new() -> Self {
        Self {
            id_to_hash: HashMap::new(),
            hash_to_id: HashMap::new(),
            preimages: HashMap::new(),
            commitments: HashMap::new(),
        }
    }
    // TODO: Implement remaining functions from js version
    // TODO add a function to provide initial hashes
}

struct Model(HashMap<String, u8>);

impl Model {
    fn set(&mut self, commitment_id: String, value: u8) {
        let prev_value = self.0.get(&commitment_id);

        // Check for equivocation
        if prev_value != None && *prev_value.unwrap() != value {
            panic!("Value of {commitment_id} is already set to a different value: {value} in model: {}", *prev_value.unwrap());
        }

        self.0.insert(commitment_id, value);
    }

    fn get_u160(&self, identifier: String) -> U160 {
        let mut result = [0; 5];
        for i in 0..5 {
            let childId = format!("{}_{}", identifier, 5 - i);
            let value = self.get_u32_endian(childId);
            result[4 - i] = value;
        }
        result
    }

    fn get_u32(&self, identifier: String) -> u32 {
        let mut result: u32 = 0;
        for i in 0..4 {
            let childId = format!("{}_byte{}", identifier, 3 - i);
            let value: u32 = self.get_u8(childId).into();
            result <<= 8;
            result += value
        }
        result
    }

    // TODO: it seems like code smell that we need this method at all. Can we get rid of it?
    fn get_u32_endian(&self, identifier: String) -> u32 {
        let mut result: u32 = 0;
        for i in 0..4 {
            let childId = format!("{}_byte{}", identifier, i);
            let value: u32 = self.get_u8(childId).into();
            result <<= 8;
            result += value
        }
        result
    }

    fn get_u8(&self, identifier: String) -> u8 {
        let mut result = 0;
        for i in 0..4 {
            let childId = format!("{}_{}", identifier, 3 - i);
            let value = self.get_u2(childId);
            result <<= 2;
            result += value
        }
        result
    }

    fn get_u2(&self, identifier: String) -> u8 {
        *self.0.get(&identifier).unwrap()
    }

    fn get_u1(&self, identifier: String) -> u8 {
        *self.0.get(&identifier).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use crate::scripts::actor::Actor;

    use super::{Opponent, Player};

    #[test]
    fn test_preimage() {
        let opponent = Opponent::new();
        let mut player = Player::new(
            &String::from("d898098e09898a0980989b980809809809f09809884324874302975287524398"),
            &opponent,
        );
        let preimage = player.preimage("TRACE_RESPONSE_0_5_byte0", Some(3), 3);

        assert_eq!(
            hex::encode(preimage),
            "7e85b1014de4146f534005c74f309220fe8a5a3c"
        )
    }
}
