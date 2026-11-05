use super::*;

#[derive(FromZeroes,FromBytes,AsBytes)]
#[repr(C)]
struct Header {first:u32,second:u32}

#[test]
fn layout_reads_refuse_bad_lengths_and_preserve_exact_bytes() {
    let bytes=[1,2,3,4,5,6,7,8];
    let header=Header::read_from(&bytes).unwrap();
    assert_eq!(header.as_bytes(),bytes);
    for size in 0..bytes.len() {assert!(Header::read_from(&bytes[..size]).is_none());}
    assert!(Header::read_from(&[0;9]).is_none());
}

#[test]
fn layout_references_reject_misalignment_and_writes_stay_in_the_slice() {
    let mut words=[0_u32;4];
    let bytes=words.as_bytes_mut();
    assert!(Ref::<_,Header>::new(&bytes[1..9]).is_none());
    {
        let mut header=Ref::<_,Header>::new(&mut bytes[4..12]).unwrap();
        header.first=0x01020304;header.second=0xaabbccdd;
    }
    assert_eq!(words,[0,0x01020304,0xaabbccdd,0]);
    let header=Header::new_zeroed();assert_eq!(header.as_bytes(),[0;8]);
}
