use std::net::IpAddr;
use std::{net::SocketAddr, num::ParseIntError};
use anyhow::{Result, bail, Context};
use pcap_file::pcap::PcapReader;
use pcap_file::DataLink;
use etherparse::TransportSlice;
use etherparse::SlicedPacket;
use etherparse::InternetSlice;

pub mod hex;
use crate::hex::BinStrLine;

fn main() {
    decode_srtp_pcap().unwrap();
}


fn decode_srtp_pcap() -> Result<()> {

    // let path = "/Users/simon/Downloads/mini.pcap";
    // let max_packets = 300; // 515;
    // let file_in = File::open(path).with_context(||"Error opening file")?;

    let file_in = include_bytes!("../data/mini.pcap");
    let max_packets = 300; 

    // srtp decoder params: 
    // -d "192.168.0.152:11001=}192.168.0.177:52052,df669b6820f1f3ae23108dc232b0aacb8d0b5425113aea2c649502c20eaa" 
    // -d "192.168.0.177:52052=}192.168.0.152:11001,ec4540206f84c8ba2c354ad4ff301e70ad406167211c656e278246856e16" 

    let src_addr: SocketAddr = "192.168.0.177:52052".parse()?;
    let dst_addr: SocketAddr = "192.168.0.152:11001".parse()?;

    let key = decode_hex("ec4540206f84c8ba2c354ad4ff301e70ad406167211c656e278246856e16")
    .with_context(||"parse srtp key fail")?;

    // let raw_data = [0_u8; 100];

    let mut session = srtp::Session::with_inbound_template(srtp::StreamPolicy {
        key: &key,
        rtp: srtp::CryptoPolicy::aes_cm_128_hmac_sha1_80(),
        rtcp: srtp::CryptoPolicy::aes_cm_128_hmac_sha1_80(),
        ..Default::default()
    }).with_context(||"fail to init srtp session")?;

    
    
    let mut pcap_reader = PcapReader::new(&file_in[..])
    .with_context(||"create PcapReader fail")?;
    
    let header = pcap_reader.header();
    println!("{:?}", header);
    println!("");

    if header.datalink != DataLink::ETHERNET {
        bail!("only support DataLink::ETHERNET but got [{:?}]", header.datalink);
    }

    let mut stati = Stati::default();

    while stati.read_packets < max_packets {

        let pkt = match pcap_reader.next_packet() {
            Some(pkt) => pkt,
            None => break,
        };

        let pkt = pkt.with_context(||"read packet fail")?;
        stati.read_packets +=1;
        
        let sliced_packet = SlicedPacket::from_ethernet(&pkt.data)
        .with_context(|| format!("No.{} packet parsed as ethernet fail", stati.read_packets))?;

        // println!("No.{} packet data {:?}", pkt.data.dump_bin(), num_packets);
        // println!("  payload {:?}", sliced_packet.payload.dump_bin());
        
        if match_udp_addr(&src_addr, &dst_addr, &sliced_packet) {
            stati.matched_packets += 1;
            println!("No.{} packet matched, {:?}", stati.read_packets, pkt.data.dump_bin());

            let first_byte = sliced_packet.payload[0];

            if first_byte > 127 && first_byte < 192 { // rtp/rtcp
                
                let pt = sliced_packet.payload[1] & 0x7f;
            let is_rtcp = pt > 63 && pt < 96;
            

            let mut cypher_packet = sliced_packet.payload.to_vec();

            if !is_rtcp {
                match session.unprotect(&mut cypher_packet) {
                    Ok(()) => {
                        println!("  Success unprotected rtp packet");
                        stati.decoded_ok_rtp_packets += 1;
                    },
                    Err(err) => {
                        println!("  Error unprotecting rtp packet: {}", err);
                        stati.decoded_err_rtp_packets += 1;
                    },
                }
            } else {
                match session.unprotect_rtcp(&mut cypher_packet) {
                    Ok(()) => {
                        println!("  Success unprotected rtcp packet");
                        stati.decoded_ok_rtcp_packets += 1;
                    },
                    Err(err) => {
                        println!("  Error unprotecting rtcp packet: {}", err);
                        stati.decoded_err_rtcp_packets += 1;
                    },
                }
            }


            } else if first_byte > 19 && first_byte < 64 { // dtls
                stati.dtls_packets += 1;

            } else if first_byte < 2 { // stun
                stati.stun_packets += 1;

            } else { // unknown
                stati.unknown_packets += 1;
            }

            println!("");
        }
    }

    println!("final: {:#?}", stati);
    
    Ok(())
}

#[derive(Debug, Default)]
pub struct Stati {
    pub read_packets: u64,
    
    pub matched_packets: u64,
    pub stun_packets: u64,
    pub dtls_packets: u64,
    pub unknown_packets: u64,
    
    pub decoded_ok_rtp_packets: u64,
    pub decoded_err_rtp_packets: u64,

    pub decoded_ok_rtcp_packets: u64,
    pub decoded_err_rtcp_packets: u64,
}

#[derive(Debug)]
struct IpAddrPair {
    src: IpAddr,
    dst: IpAddr,
}



fn match_udp_addr(src: &SocketAddr, dst: &SocketAddr, packet: &SlicedPacket) -> bool {
    
    if let Some(ip) = &packet.ip {

        let pair = to_ipaddr(ip);
        // println!(" ip-pair {:?}", pair);

        if pair.src != src.ip() {
            return false;
        }

        if pair.dst != dst.ip() {
            return false;
        }
    }

    if let Some(transport) = &packet.transport {
        if let TransportSlice::Udp(header) = transport {
            // println!(" port-pair {:?}", (header.source_port(), header.destination_port()));

            if header.source_port() != src.port() {
                return false;
            }

            if header.destination_port() != dst.port() {
                return false;
            }

            return true;
        }
    }

    false
}

fn to_ipaddr(ip: &InternetSlice) -> IpAddrPair {
    match ip {
        InternetSlice::Ipv4(header, _ext) => {
            IpAddrPair{
                src: IpAddr::V4(header.source_addr()) ,
                dst: IpAddr::V4(header.destination_addr()) ,
            }
        },
        InternetSlice::Ipv6(header, _ext) => {
            IpAddrPair{
                src: IpAddr::V6(header.source_addr()) ,
                dst: IpAddr::V6(header.destination_addr()) ,
            }
        },
    }
}

fn decode_hex(s: &str) -> Result<Vec<u8>, ParseIntError> {
    // from https://stackoverflow.com/questions/52987181/how-can-i-convert-a-hex-string-to-a-u8-slice
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16))
        .collect()
}





