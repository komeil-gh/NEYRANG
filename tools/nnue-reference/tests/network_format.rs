use neyrang_nnue_reference::{
    FEATURE_SET_CHESS768, HEADER_SIZE, HIDDEN_SIZE, INPUT_FEATURES, Network, NetworkError,
    NetworkParameters,
};

fn fixture_network() -> Network {
    let feature_weights = (0..INPUT_FEATURES * HIDDEN_SIZE)
        .map(|index| (index as i16 % 17) - 8)
        .collect();
    let feature_bias = (0..HIDDEN_SIZE)
        .map(|index| (index as i16 % 11) - 5)
        .collect();
    let output_weights = (0..2 * HIDDEN_SIZE)
        .map(|index| (index as i16 % 7) - 3)
        .collect();

    Network::new(
        NetworkParameters {
            activation_quant: 255,
            output_quant: 64,
            centipawn_scale: 400,
        },
        feature_weights,
        feature_bias,
        output_weights,
        23,
    )
    .expect("fixture shape is valid")
}

#[test]
fn network_round_trips_through_versioned_little_endian_format() {
    let network = fixture_network();
    let bytes = network.to_bytes();

    assert_eq!(&bytes[..8], b"NEYRANGNNUE");
    assert_eq!(
        u16::from_le_bytes([bytes[10], bytes[11]]),
        FEATURE_SET_CHESS768
    );
    assert_eq!(
        u16::from_le_bytes([bytes[12], bytes[13]]) as usize,
        HIDDEN_SIZE
    );
    assert_eq!(Network::from_bytes(&bytes), Ok(network));
}

#[test]
fn loader_rejects_incompatible_headers_before_inference() {
    let bytes = fixture_network().to_bytes();

    let mut bad_magic = bytes.clone();
    bad_magic[0] ^= 0xff;
    assert_eq!(Network::from_bytes(&bad_magic), Err(NetworkError::BadMagic));

    let mut bad_version = bytes.clone();
    bad_version[8..10].copy_from_slice(&2_u16.to_le_bytes());
    assert_eq!(
        Network::from_bytes(&bad_version),
        Err(NetworkError::UnsupportedVersion(2))
    );

    let mut zero_quant = bytes;
    zero_quant[14..16].copy_from_slice(&0_u16.to_le_bytes());
    assert_eq!(
        Network::from_bytes(&zero_quant),
        Err(NetworkError::InvalidQuantization)
    );
}

#[test]
fn loader_rejects_truncation_trailing_bytes_and_payload_corruption() {
    let bytes = fixture_network().to_bytes();

    let mut truncated = bytes.clone();
    truncated.pop();
    assert!(matches!(
        Network::from_bytes(&truncated),
        Err(NetworkError::LengthMismatch { .. })
    ));

    let mut trailing = bytes.clone();
    trailing.push(0);
    assert!(matches!(
        Network::from_bytes(&trailing),
        Err(NetworkError::LengthMismatch { .. })
    ));

    let mut corrupt = bytes;
    corrupt[HEADER_SIZE] ^= 1;
    assert!(matches!(
        Network::from_bytes(&corrupt),
        Err(NetworkError::ChecksumMismatch { .. })
    ));
}

#[test]
fn constructor_rejects_wrong_tensor_shapes() {
    let result = Network::new(
        NetworkParameters {
            activation_quant: 255,
            output_quant: 64,
            centipawn_scale: 400,
        },
        vec![0; INPUT_FEATURES * HIDDEN_SIZE - 1],
        vec![0; HIDDEN_SIZE],
        vec![0; 2 * HIDDEN_SIZE],
        0,
    );

    assert!(matches!(result, Err(NetworkError::ShapeMismatch { .. })));
}
