use super::*;
#[path = "auth_rng.rs"]
mod rng;

// RFC9497 Appendix A.1.1: public Ristretto255/SHA512 base-OPRF vectors.
#[test]
fn published_oprf_vectors_match_key_blinding_evaluation_and_final_output() {
    let key = derive_key(&[0xa3; 32], b"test key").unwrap();
    assert_eq!(
        key.to_bytes(),
        rng::decode("5ebcea5ee37023ccb9fc2d2019f9d7737be85591ae8652ffa9ef0f4d37063b0e")
    );
    for (input, blinded, evaluation, output) in [
        (
            vec![0],
            "609a0ae68c15a3cf6903766461307e5c8bb2f95e7e6550e1ffa2dc99e412803c",
            "7ec6578ae5120958eb2db1745758ff379e77cb64fe77b0b2d8cc917ea0869c7e",
            "527759c3d9366f277d8c6020418d96bb393ba2afb20ff90df23fb7708264e2f3ab9135e3bd69955851de4b1f9fe8a0973396719b7912ba9ee8aa7d0b5e24bcf6",
        ),
        (
            vec![0x5a; 17],
            concat!("da27ef466870f5f15", "296299850aa08862", "9945a17d1f5b7f5f", "f043f76b3c06418"),
            "b4cbf5a4f1eeda5a63ce7b77c7d23f461db3fcab0dd28e4e17cecb5c90d02c25",
            "f4a74c9c592497375e796aa837e907b1a045d34306a749db9f34221f7e750cb4f2a6413a6bf6fa5e19ba6348eb673934a722a7ede2e7621306d18951e7cf2c73",
        ),
    ] {
        let scalar = decode_scalar(rng::decode(
            "64d37aed22a27f5191de1c1d69fadb899d8862b58eb4220029e036ec4c1f6706",
        ))
        .unwrap();
        let uniform = expand_message(&[&input], b"HashToGroup-");
        let element = RistrettoPoint::from_uniform_bytes(&uniform) * *scalar;
        assert_eq!(element.compress().to_bytes(), rng::decode::<32>(blinded));
        let evaluated = element * *key;
        assert_eq!(
            evaluated.compress().to_bytes(),
            rng::decode::<32>(evaluation)
        );
        assert_eq!(
            *Blind { scalar }.finalize(&input, evaluated).unwrap(),
            rng::decode::<64>(output)
        );
    }
}
#[test]
fn invalid_identity_points_noncanonical_and_zero_scalars_are_rejected() {
    for bytes in [[0; 32], [255; 32]] {
        assert!(decode_point(bytes).is_err());
        assert!(decode_scalar(bytes).is_err());
    }
    let order = rng::decode("edd3f55c1a631258d69cf7a2def9de1400000000000000000000000000000010");
    assert!(decode_scalar(order).is_err());
    let mut source = rng::Stream::new(13);
    let scalar = random_scalar(&mut source);
    assert_ne!(*scalar, Scalar::ZERO);
    assert_eq!(source.bytes, 64);
    assert_eq!(*decode_scalar(scalar.to_bytes()).unwrap(), *scalar);
    assert_eq!(
        decode_point(public_key(&scalar).compress().to_bytes()).unwrap(),
        public_key(&scalar)
    );
}
#[test]
fn oprf_input_length_limit_and_domain_separation_are_preserved() {
    let mut source = rng::Stream::new(5);
    let (blind, element) = Blind::new(b"", &mut source);
    let key = derive_key(&[3; 32], b"test").unwrap();
    let evaluated = element * *key;
    assert!(blind.finalize(&vec![0; 65535], evaluated).is_ok());
    assert!(blind.finalize(&vec![0; 65536], evaluated).is_err());
    assert_ne!(
        *derive_key(&[3; 32], b"OPAQUE-DeriveKeyPair").unwrap(),
        *derive_key(&[3; 32], b"OPAQUE-DeriveDiffieHellmanKeyPair").unwrap()
    );
}

#[test]
fn a_zero_random_draw_is_rejected_before_a_nonzero_secret_is_exposed() {
    assert!(std::mem::needs_drop::<NonZeroScalar>());
    struct ZeroFirst {
        calls: usize,
        stream: rng::Stream,
    }
    impl CryptoRng for ZeroFirst {}
    impl RngCore for ZeroFirst {
        fn next_u32(&mut self) -> u32 {
            let mut bytes = [0; 4];
            self.fill_bytes(&mut bytes);
            u32::from_le_bytes(bytes)
        }
        fn next_u64(&mut self) -> u64 {
            let mut bytes = [0; 8];
            self.fill_bytes(&mut bytes);
            u64::from_le_bytes(bytes)
        }
        fn fill_bytes(&mut self, bytes: &mut [u8]) {
            if self.calls == 0 {
                bytes.fill(0);
            } else {
                self.stream.fill_bytes(bytes);
            }
            self.calls += 1;
        }
        fn try_fill_bytes(&mut self, bytes: &mut [u8]) -> Result<(), rand_core::Error> {
            self.fill_bytes(bytes);
            Ok(())
        }
    }
    let mut source = ZeroFirst {
        calls: 0,
        stream: rng::Stream::new(17),
    };
    let scalar = random_scalar(&mut source);
    assert_eq!(source.calls, 2);
    assert_ne!(*scalar, Scalar::ZERO);
}

#[test]
fn blinding_state_owns_only_the_scalar_not_a_duplicate_public_point() {
    assert_eq!(
        std::mem::size_of::<Blind>(),
        std::mem::size_of::<NonZeroScalar>()
    );
}

#[test]
fn fixed_base_public_keys_match_the_original_constant_time_multiplication() {
    let mut source = rng::Stream::new(919);
    for _ in 0..128 {
        let scalar = random_scalar(&mut source);
        assert_eq!(
            public_key(&scalar),
            curve25519_dalek::constants::RISTRETTO_BASEPOINT_POINT * *scalar
        );
    }
    for value in [1, 2, 7, 128, 255] {
        let mut bytes = [0; 32];
        bytes[0] = value;
        let scalar = decode_scalar(bytes).unwrap();
        assert_eq!(
            public_key(&scalar),
            curve25519_dalek::constants::RISTRETTO_BASEPOINT_POINT * *scalar
        );
    }
}
