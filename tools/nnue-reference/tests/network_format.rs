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

#[test]
fn pinned_bullet_tensor_stream_imports_without_padding_ambiguity() {
    let parameters = NetworkParameters {
        activation_quant: 255,
        output_quant: 64,
        centipawn_scale: 400,
    };
    let bytes = fixture_bullet_quantised();

    assert_eq!(
        Network::from_bullet_quantised(&bytes, parameters),
        Ok(fixture_network())
    );
}

#[test]
fn bullet_import_rejects_wrong_length_and_padding() {
    let parameters = NetworkParameters {
        activation_quant: 255,
        output_quant: 64,
        centipawn_scale: 400,
    };
    let bytes = fixture_bullet_quantised();

    assert!(matches!(
        Network::from_bullet_quantised(&bytes[..bytes.len() - 1], parameters),
        Err(NetworkError::LengthMismatch { .. })
    ));

    let mut corrupt_padding = bytes;
    let last = corrupt_padding.len() - 1;
    corrupt_padding[last] ^= 1;
    assert_eq!(
        Network::from_bullet_quantised(&corrupt_padding, parameters),
        Err(NetworkError::InvalidBulletPadding)
    );
}

fn fixture_bullet_quantised() -> Vec<u8> {
    let mut bytes = Vec::new();
    for index in 0..INPUT_FEATURES * HIDDEN_SIZE {
        bytes.extend_from_slice(&((index as i16 % 17) - 8).to_le_bytes());
    }
    for index in 0..HIDDEN_SIZE {
        bytes.extend_from_slice(&((index as i16 % 11) - 5).to_le_bytes());
    }
    for index in 0..2 * HIDDEN_SIZE {
        bytes.extend_from_slice(&((index as i16 % 7) - 3).to_le_bytes());
    }
    bytes.extend_from_slice(&23_i32.to_le_bytes());
    let padding = 64 - bytes.len() % 64;
    for index in 0..padding {
        bytes.push(b"bullet"[index % 6]);
    }
    bytes
}
