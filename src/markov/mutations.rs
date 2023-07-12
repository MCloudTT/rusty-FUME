use crate::markov::BOF_CHANCE;
use rand::prelude::ThreadRng;
use rand::Rng;

pub fn inject(packet: &mut Vec<u8>, rng: &mut ThreadRng) {
    if rng.gen_range(0f32..1f32) > BOF_CHANCE {
        inject_single(packet, rng);
    } else {
        inject_bof(packet, rng);
    }
}

fn inject_bof(packet: &mut Vec<u8>, rng: &mut ThreadRng) {
    let idx = rng.gen_range(0..packet.len());
    let byte_length = rng.gen_range(packet.len()..packet.len() * 4);
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
    fn test_inject() {
        let mut rng = thread_rng();
        for _ in 0..100 {
            let mut packet = vec![0; 10];
            println!("Input packet: {:?}", packet);
            inject(&mut packet, &mut rng);
            println!("Output packet: {:?}", packet);
            assert!(packet.len() > 10);
            assert!(packet.len() < 50);
        }
    }
    #[test]
    fn test_inject_bof() {
        let mut rng = thread_rng();
        let mut packet = vec![0; 10];
        println!("Input packet: {:?}", packet);
        inject_bof(&mut packet, &mut rng);
        println!("Output packet: {:?}", packet);
        assert!(packet.len() > 10);
        assert!(packet.len() < 50);
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
