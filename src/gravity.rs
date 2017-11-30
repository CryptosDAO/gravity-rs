use hash;
use hash::Hash;
use address;
use prng;
use merkle;
use pors;
use subtree;
use config::*;

pub struct SecKey {
    seed: Hash,
    salt: Hash,
    cache: merkle::MerkleTree,
}
pub struct PubKey {
    pub h: Hash,
}
#[derive(Default)]
pub struct Signature {
    pors_sign: pors::Signature,
    subtrees: [subtree::Signature; GRAVITY_D],
    auth_c: [Hash; GRAVITY_C],
}

impl SecKey {
    pub fn new(random: &[u8; 64]) -> Self {
        let mut sk = SecKey {
            seed: Hash { h: *array_ref![random, 0, 32] },
            salt: Hash { h: *array_ref![random, 32, 32] },
            cache: merkle::MerkleTree::new(GRAVITY_C),
        };

        {
            let leaves = sk.cache.leaves();
            let layer = 0u32;

            let prng = prng::Prng::new(&sk.seed);
            let subtree_sk = subtree::SecKey::new(&prng);
            for i in 0..GRAVITY_CCC {
                let address = address::Address::new(layer, (MERKLE_HHH * i) as u64);
                let pk = subtree_sk.genpk(&address);
                leaves[i] = pk.h;
            }
        }

        sk.cache.generate();
        sk
    }

    pub fn genpk(&self) -> PubKey {
        PubKey { h: self.cache.root() }
    }

    pub fn sign_hash(&self, msg: &Hash) -> Signature {
        let mut sign: Signature = Default::default();

        let prng = prng::Prng::new(&self.seed);
        let (mut address, mut h, pors_sign) = pors::sign(&prng, &self.salt, msg);
        sign.pors_sign = pors_sign;

        let subtree_sk = subtree::SecKey::new(&prng);
        for i in 0..GRAVITY_D {
            address.next_layer();
            let (root, subtree_sign) = subtree_sk.sign(&address, &h);
            h = root;
            sign.subtrees[i] = subtree_sign;
            address.shift(MERKLE_H); // Update instance
        }

        let index = address.get_instance();
        self.cache.gen_auth(&mut sign.auth_c, index);

        sign
    }

    pub fn sign_bytes(&self, msg: &[u8]) -> Signature {
        let h = hash::long_hash(msg);
        self.sign_hash(&h)
    }
}

impl PubKey {
    fn verify_hash(&self, sign: &Signature, msg: &Hash) -> bool {
        if let Some(h) = sign.extract_hash(msg) {
            self.h == h
        } else {
            false
        }
    }

    pub fn verify_bytes(&self, sign: &Signature, msg: &[u8]) -> bool {
        let h = hash::long_hash(msg);
        self.verify_hash(sign, &h)
    }
}

impl Signature {
    fn extract_hash(&self, msg: &Hash) -> Option<Hash> {
        if let Some((mut address, mut h)) = self.pors_sign.extract(msg) {
            for i in 0..GRAVITY_D {
                address.next_layer();
                h = self.subtrees[i].extract(&address, &h);
                address.shift(MERKLE_H);
            }

            let index = address.get_instance();
            merkle::merkle_compress_auth(&mut h, &self.auth_c, GRAVITY_C, index);
            Some(h)
        } else {
            None
        }
    }

    pub fn serialize(&self, output: &mut Vec<u8>) {
        self.pors_sign.serialize(output);
        for t in self.subtrees.iter() {
            t.serialize(output);
        }
        for x in self.auth_c.iter() {
            x.serialize(output);
        }
    }

    pub fn deserialize<'a, I>(it: &mut I) -> Option<Self>
    where
        I: Iterator<Item = &'a u8>,
    {
        let mut sign: Signature = Default::default();
        sign.pors_sign = pors::Signature::deserialize(it)?;
        for i in 0..GRAVITY_D {
            sign.subtrees[i] = subtree::Signature::deserialize(it)?;
        }
        for i in 0..GRAVITY_C {
            sign.auth_c[i] = Hash::deserialize(it)?;
        }
        Some(sign)
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sign_verify() {
        let mut random = [0u8; 64];
        for i in 0..64 {
            random[i] = i as u8;
        }

        let sk = SecKey::new(&random);
        let pk = sk.genpk();
        let msg = hash::tests::HASH_ELEMENT;
        let sign = sk.sign_hash(&msg);
        assert!(pk.verify_hash(&sign, &msg));
    }

    // TODO: check config parameters in these tests.
    #[test]
    fn test_genkey_zeros() {
        let random: [u8; 64] = [0u8; 64];
        let pkh: [u8; 32] = *b"\x57\x03\x58\x87\x1a\x7a\x2c\xfe\
                               \x1e\xab\xf1\x3b\x4c\x11\x3a\x81\
                               \xce\x08\x9a\x2c\x02\x04\xa3\xbb\
                               \xc4\x4d\xd7\xb6\x94\x07\x94\x2a";

        let sk = SecKey::new(&random);
        let pk = sk.genpk();
        assert_eq!(pk.h.h, pkh);
    }

    #[test]
    fn test_sign_zeros() {
        use hex;

        let random: [u8; 64] = [0u8; 64];
        let msg: [u8; 32] = *b"\x00\x01\x02\x03\x04\x05\x06\x07\
                               \x08\x09\x0a\x0b\x0c\x0d\x0e\x0f\
                               \x10\x11\x12\x13\x14\x15\x16\x17\
                               \x18\x19\x1a\x1b\x1c\x1d\x1e\x1f";

        let mut hex: Vec<u8> = vec![];
        for x in include_str!("../test_files/test_sign_zero.hex").split_whitespace() {
            hex.extend(x.bytes())
        }
        let expect: Vec<u8> = hex::decode(hex).unwrap();

        let sk = SecKey::new(&random);
        let sign = sk.sign_bytes(&msg);
        let mut sign_bytes = Vec::<u8>::new();
        sign.serialize(&mut sign_bytes);
        assert_eq!(sign_bytes, expect);
    }

    // TODO: KATs
}
