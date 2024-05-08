// Probably the least efficient SHA-256 implementation I have ever written,
// but with this no external dependencies will be needed.

use std::io;
use std::io::prelude::*;
use std::fs;
use std::path::Path;

const BLOCK_SIZE : usize = 512;
const STATE_SIZE : usize = 256;

pub type State = [u32; STATE_SIZE / 32];
type Block = [u8; BLOCK_SIZE / 8];

// SHA-256 constants

#[allow(non_upper_case_globals)]
const h0: State = [
    0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a,
    0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
];

const K: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
    0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
    0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
    0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
    0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
    0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
    0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
    0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
    0xc67178f2
];

// helper macros/functions

// normal add with wrap-around
macro_rules! addw {
    ($a:expr,$($b:tt)*)=>{ ($a).wrapping_add(  addw!($($b)*) ) };
    ($a:expr)=> { $a };
}

// convert from bytes to word array
fn to_u32(b: &Block, s : &mut [u32]) {
    for (i, c) in b.chunks_exact(4).enumerate() {
        s[i] = u32::from_be_bytes(c.try_into().unwrap() );
    }
}


// SHA-256 functions
fn sig0(i : u32) -> u32 {
    i.rotate_right(7) ^ i.rotate_right(18) ^ (i >> 3)
}
fn sig1(i : u32) -> u32 {
    i.rotate_right(17) ^ i.rotate_right(19) ^ (i >> 10)
}
fn sum0(i : u32) -> u32 {
    i.rotate_right(2) ^ i.rotate_right(13) ^ i.rotate_right(22)
}
fn sum1(i : u32) -> u32 {
    i.rotate_right(6) ^ i.rotate_right(11) ^ i.rotate_right(25)
}
fn choice(x : u32, y : u32, z : u32) -> u32 {
    (x & y) ^ ((!x) & z)
}
fn major(x : u32, y : u32, z : u32) -> u32 {
    (x & y) ^ (x & z) ^ (y & z)
}

// SHA-256 implementation
struct SHA256 {
    state : State,
    length : u64,
}

impl SHA256 {
    fn new() -> Self {
        Self { state: h0, length: 0 }
    }

    fn process_full(&mut self, buffer: &Block) {
        let mut m : [u32; 64] = [0; 64];
        to_u32(buffer, &mut m[0..BLOCK_SIZE / 32]);

        for i in 16..64 {
            m[i] = addw!( sig1(m[i -2]), m[i - 7], sig0(m[i - 15]), m[i -16]);
        }

        let mut a = self.state[0];
        let mut b = self.state[1];
        let mut c = self.state[2];
        let mut d = self.state[3];
        let mut e = self.state[4];
        let mut f = self.state[5];
        let mut g = self.state[6];
        let mut h = self.state[7];

        for i in 0..64 {
            let t1 = addw!( h, sum1(e), choice(e,f,g), K[i], m[i]);
            let t2 = addw!(sum0(a), major(a,b,c));
            h = g;
            g = f;
            f = e;
            e = addw!(d, t1);
            d = c;
            c = b;
            b = a;
            a = addw!(t1, t2);
        }

        self.state[0] = addw!(self.state[0], a);
        self.state[1] = addw!(self.state[1], b);
        self.state[2] = addw!(self.state[2], c);
        self.state[3] = addw!(self.state[3], d);
        self.state[4] = addw!(self.state[4], e);
        self.state[5] = addw!(self.state[5], f);
        self.state[6] = addw!(self.state[6], g);
        self.state[7] = addw!(self.state[7], h);
    }


    fn process(&mut self, buffer: &Block) {
        self.length += (BLOCK_SIZE / 8) as u64;
        self.process_full(buffer);
    }

    fn finalize(&mut self, buffer: &mut Block, size: usize) -> [u32; 8] {
        self.length += size as u64;
        let mut cur = self.length as usize % (BLOCK_SIZE / 8);

        if cur >= (BLOCK_SIZE - 64 - 1) / 8 {
            buffer[cur] = 0x80;
            cur += 1;
            for i in cur.. BLOCK_SIZE / 8 {
                buffer[i] = 0x00;
            }
            self.process_full(buffer);
            cur = 0;
        } else {
            buffer[cur] = 0x80;
            cur += 1;
        }

        for i in cur.. (BLOCK_SIZE - 64) / 8 {
            buffer[i] = 0x00;
        }

        let length = (self.length * 8).to_be_bytes();
        buffer[(BLOCK_SIZE - 64) / 8..].copy_from_slice(&length[..]);

        self.process_full(buffer);
        self.state
    }
}

pub fn from_file(path: &Path) -> io::Result<State> {
    let file = fs::File::open(path)?;
    let mut file = io::BufReader::new(file);

    let mut hash = SHA256::new();
    let mut buffer : Block = [0 ; BLOCK_SIZE / 8];
    loop {
        let n = file.read(&mut buffer)?;
        if n < BLOCK_SIZE / 8 {
            return Ok( hash.finalize(&mut buffer, n) );
        }
        hash.process(&buffer);
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_conversion() {
	let block : Block = [
	    0,1,2,3, 4,5,6,7, 8,9,10,11, 12,13,14,15,
	    16,17,18,19,20,21,22,23,24,25,26,27,28,29,30,31,
	    0,1,2,3, 4,5,6,7, 8,9,10,11, 12,13,14,15,
	    16,17,18,19,20,21,22,23,24,25,26,27,28,29,30,31,

	];
	let expected : [u32; 16] = [
	    0x00010203, 0x04050607, 0x08090a0b, 0x0c0d0e0f,
	    0x10111213, 0x14151617, 0x18191a1b, 0x1c1d1e1f,
	    0x00010203, 0x04050607, 0x08090a0b, 0x0c0d0e0f,
	    0x10111213, 0x14151617, 0x18191a1b, 0x1c1d1e1f,
	];

	let mut got: [u32; 16] = [0; 16];
	to_u32(&block, &mut got);

	assert_eq!(expected, got);
    }

    #[test]
    fn test_addw() {
	assert_eq!( 6, addw!(1 as u32, 2 as u32, 3 as u32));
	assert_eq!( 0, addw!(0xffff_ffff as u32, 1 as u32));
    }

    #[test]
    fn test_hash() {
	// case 1
	let input = "abc".as_bytes();
	let hash = [
	    0xba7816bf, 0x8f01cfea, 0x414140de, 0x5dae2223,
	    0xb00361a3, 0x96177a9c, 0xb410ff61, 0xf20015ad];

	let mut sha = SHA256::new();
	let mut block : Block = [0; BLOCK_SIZE / 8];
	block[0..input.len()].copy_from_slice(input);
	let got = sha.finalize(&mut block, input.len());
	assert_eq!(hash, got);

	// case 2
	let input = "abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq".as_bytes();
	let hash  = [
	    0x248d6a61, 0xd20638b8, 0xe5c02693, 0x0c3e6039,
	    0xa33ce459, 0x64ff2167, 0xf6ecedd4, 0x19db06c1];

	let mut sha = SHA256::new();
	let mut block : Block = [0; BLOCK_SIZE / 8];
	block[0..input.len()].copy_from_slice(input);
	println!("block: {} {:?}", input.len(), block);
	let got = sha.finalize(&mut block, input.len());
	assert_eq!(hash, got);
    }

    #[test]
    fn test_file() {
	let hash = [
	    0x17e682f0, 0x60b5f8e4, 0x7ea04c5c, 0x4855908b,
	    0x0a5ad612, 0x022260fe, 0x50e11ecb, 0x0cc0ab76,
	];
	let got = from_file(Path::new("test/a") ).expect("Could not hash file");
	assert_eq!(hash, got);
    }
}
