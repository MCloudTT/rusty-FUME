use crate::markov::BOF_CHANCE;
use rand::distributions::Standard;
use rand::prelude::{Distribution, ThreadRng};
use rand::Rng;

pub fn inject(packet: &mut Vec<u8>, rng: &mut ThreadRng, inject_type: &InjectType) {
    match inject_type {
        InjectType::Single => inject_single(packet, rng),
        InjectType::BOF => inject_bof(packet, rng),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum InjectType {
    Single,
    BOF,
}

impl Distribution<InjectType> for Standard {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> InjectType {
        match rng.gen_range(0..2) {
            0 => InjectType::Single,
            1 => InjectType::BOF,
            _ => unreachable!(),
        }
    }
}

fn inject_bof(packet: &mut Vec<u8>, rng: &mut ThreadRng) {
    let idx = rng.gen_range(0..packet.len());
    let byte_length = rng.gen_range(packet.len()..packet.len() * 2);
    let mut bytes = vec![0; byte_length];
    rng.fill(&mut bytes[..]);
    packet.splice(idx..idx, bytes);
}

fn inject_single(packet: &mut Vec<u8>, rng: &mut ThreadRng) {
    let idx = rng.gen_range(0..packet.len());
    let byte = rng.gen::<u8>();
    packet.insert(idx, byte);
}

pub fn delete(packet: &mut Vec<u8>, rng: &mut ThreadRng) {
    let idx = rng.gen_range(0..packet.len());
    packet.remove(idx);
}

pub fn swap(packet: &mut Vec<u8>, rng: &mut ThreadRng) {
    let idx = rng.gen_range(0..packet.len());
    let byte = rng.gen::<u8>();
    packet[idx] = byte;
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::prelude::thread_rng;
    #[test]
    fn test_inject_bof() {
        let mut rng = thread_rng();
        let mut packet = vec![0; 10];
        println!("Input packet: {:?}", packet);
        inject_bof(&mut packet, &mut rng);
        println!("Output packet: {:?}", packet);
        assert!(packet.len() > 10);
        assert!(packet.len() < 30);
    }

    #[test]
    fn test_inject_single() {
        let mut rng = thread_rng();
        let mut packet = vec![0; 10];
        println!("Input packet: {:?}", packet);
        inject_single(&mut packet, &mut rng);
        println!("Output packet: {:?}", packet);
        assert_eq!(packet.len(), 11);
    }

    #[test]
    fn test_swap() {
        let mut rng = thread_rng();
        let mut packet = vec![0; 10];
        println!("Input packet: {:?}", packet);
        swap(&mut packet, &mut rng);
        println!("Output packet: {:?}", packet);
        assert_eq!(packet.len(), 10);
    }

    #[test]
    fn test_delete() {
        let mut rng = thread_rng();
        let mut packet = vec![0; 10];
        println!("Input packet: {:?}", packet);
        delete(&mut packet, &mut rng);
        println!("Output packet: {:?}", packet);
        assert!(packet.len() < 10);
        assert!(packet.len() > 0);
    }
}
