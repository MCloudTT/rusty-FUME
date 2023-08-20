use crate::Packets;
use rand::distributions::Standard;
use rand::prelude::Distribution;
use rand::Rng;
use rand_xoshiro::Xoshiro256PlusPlus;

pub fn inject(packets: &mut Packets, rng: &mut Xoshiro256PlusPlus, inject_type: &InjectType) {
    let packets_size = packets.size();
    debug_assert!(packets_size > 0, "Packet size should be greater than 0");
    let packet = packets
        .inner
        .get_mut(rng.gen_range(0..packets_size))
        .unwrap();
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

fn inject_bof(packet: &mut Vec<u8>, rng: &mut Xoshiro256PlusPlus) {
    let idx = rng.gen_range(0..packet.len());
    // To fight big packets
    let byte_length = 10000 / packet.len();
    let mut bytes = vec![0; byte_length];
    rng.fill(&mut bytes[..]);
    packet.splice(idx..idx, bytes);
}

fn inject_single(packet: &mut Vec<u8>, rng: &mut Xoshiro256PlusPlus) {
    let idx = rng.gen_range(0..packet.len());
    let byte = rng.gen::<u8>();
    packet.insert(idx, byte);
}

pub fn delete(packets: &mut Packets, rng: &mut Xoshiro256PlusPlus) {
    let packets_size = packets.size();
    let packet = packets
        .inner
        .get_mut(rng.gen_range(0..packets_size))
        .unwrap();
    let idx = rng.gen_range(0..packet.len());
    packet.remove(idx);
}

pub fn swap(packets: &mut Packets, rng: &mut Xoshiro256PlusPlus) {
    let packets_size = packets.size();
    let packet = packets
        .inner
        .get_mut(rng.gen_range(0..packets_size))
        .unwrap();
    let idx = rng.gen_range(0..packet.len());
    let byte = rng.gen::<u8>();
    packet[idx] = byte;
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::prelude::thread_rng;
    /*
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
    }*/
}
